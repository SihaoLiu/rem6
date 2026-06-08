use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedCheckpointManifestSummary,
    WorkloadExpectedTrafficTraceReplaySummary, WorkloadId, WorkloadReplayPlan, WorkloadResource,
    WorkloadResourceId, WorkloadResourceKind, WorkloadResult, WorkloadRouteId,
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

#[allow(clippy::too_many_arguments)]
fn expected_trace_summary(
    route: &str,
    scheduled_count: usize,
    response_delivery_count: usize,
    memory_trace_event_count: usize,
    memory_failure_count: usize,
    control_ack_count: usize,
    control_failure_count: usize,
    sideband_event_count: usize,
) -> WorkloadExpectedTrafficTraceReplaySummary {
    WorkloadExpectedTrafficTraceReplaySummary::new(route_id(route))
        .with_minimum_scheduled_count(scheduled_count)
        .with_minimum_response_delivery_count(response_delivery_count)
        .with_minimum_memory_trace_event_count(memory_trace_event_count)
        .with_minimum_memory_failure_count(memory_failure_count)
        .with_minimum_control_ack_count(control_ack_count)
        .with_minimum_control_failure_count(control_failure_count)
        .with_minimum_sideband_event_count(sideband_event_count)
}

#[allow(clippy::too_many_arguments)]
fn actual_trace_summary(
    route: &str,
    scheduled_count: usize,
    response_delivery_count: usize,
    memory_trace_event_count: usize,
    memory_failure_count: usize,
    control_ack_count: usize,
    control_failure_count: usize,
    sideband_event_count: usize,
) -> WorkloadTrafficTraceReplaySummary {
    WorkloadTrafficTraceReplaySummary::new(route_id(route), scheduled_count)
        .with_response_delivery_count(response_delivery_count)
        .with_memory_trace_event_count(memory_trace_event_count)
        .with_memory_failure_count(memory_failure_count)
        .with_control_ack_count(control_ack_count)
        .with_control_failure_count(control_failure_count)
        .with_sideband_event_count(sideband_event_count)
}

#[test]
fn workload_manifest_records_traffic_trace_replay_summary_expectations() {
    let trace_a = expected_trace_summary("trace.a", 1, 1, 3, 0, 0, 0, 0);
    let trace_b = expected_trace_summary("trace.b", 2, 0, 2, 1, 0, 0, 1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-traffic-trace-replay"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(trace_b.clone())
            .unwrap()
            .add_expected_traffic_trace_replay_summary(trace_a.clone())
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_traffic_trace_replay_summaries(),
        &[trace_a.clone(), trace_b.clone()],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        manifest.expected_traffic_trace_replay_summaries(),
    );

    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual_trace_summary("trace.b", 2, 0, 2, 1, 0, 0, 1))
        .with_traffic_trace_replay_summary(actual_trace_summary("trace.a", 1, 1, 3, 0, 0, 0, 0));
    assert_eq!(
        result.traffic_trace_replay_summaries(),
        &[
            actual_trace_summary("trace.a", 1, 1, 3, 0, 0, 0, 0),
            actual_trace_summary("trace.b", 2, 0, 2, 1, 0, 0, 1),
        ],
    );
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_records_response_status_trace_summary_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.response"))
        .with_minimum_response_delivery_count(3)
        .with_minimum_trace_completed_response_count(1)
        .with_minimum_trace_retry_response_count(1)
        .with_minimum_trace_store_conditional_failed_response_count(1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-response-status"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0].minimum_trace_completed_response_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0].minimum_trace_retry_response_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_trace_store_conditional_failed_response_count(),
        1
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.response"), 3)
        .with_response_delivery_count(3)
        .with_trace_completed_response_count(1)
        .with_trace_retry_response_count(1)
        .with_trace_store_conditional_failed_response_count(1);
    assert_eq!(actual.trace_completed_response_count(), 1);
    assert_eq!(actual.trace_retry_response_count(), 1);
    assert_eq!(actual.trace_store_conditional_failed_response_count(), 1);

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_traffic_trace_replay_summary(actual);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_underreported_trace_response_status_summary() {
    let expected =
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.response.status"))
            .with_minimum_response_delivery_count(1)
            .with_minimum_trace_completed_response_count(1)
            .with_minimum_trace_store_conditional_failed_response_count(2);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("trace-response-status-mismatch"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.response.status"), 2)
        .with_response_delivery_count(1)
        .with_trace_completed_response_count(1)
        .with_trace_store_conditional_failed_response_count(1);
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
fn workload_manifest_records_typed_traffic_trace_sideband_expectations() {
    let expected = expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
        .with_minimum_tlb_sync_event_count(1)
        .with_minimum_trace_tlb_sync_count(1)
        .with_minimum_cache_flush_event_count(1)
        .with_minimum_trace_cache_flush_count(1)
        .with_minimum_trace_l1_invalidation_count(1)
        .with_minimum_diagnostic_print_event_count(1)
        .with_minimum_trace_diagnostic_count(1)
        .with_minimum_htm_abort_event_count(1)
        .with_minimum_trace_htm_abort_count(1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        std::slice::from_ref(&expected),
    );

    let actual = actual_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
        .with_tlb_sync_event_count(1)
        .with_trace_tlb_sync_count(1)
        .with_cache_flush_event_count(1)
        .with_trace_cache_flush_count(1)
        .with_trace_l1_invalidation_count(1)
        .with_diagnostic_print_event_count(1)
        .with_trace_diagnostic_count(1)
        .with_htm_abort_event_count(1)
        .with_trace_htm_abort_count(1);
    assert_eq!(actual.trace_htm_abort_count(), 1);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    assert_eq!(result.traffic_trace_replay_summaries(), &[actual]);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_records_control_failure_source_expectations() {
    let expected = expected_trace_summary("trace.control.failure.sources", 5, 0, 0, 0, 0, 5, 0)
        .with_minimum_sync_control_failure_count(1)
        .with_minimum_tlb_control_failure_count(1)
        .with_minimum_cache_control_failure_count(1)
        .with_minimum_htm_control_failure_count(1)
        .with_minimum_diagnostic_control_failure_count(1)
        .with_minimum_control_failure_invalid_destination_count(4)
        .with_minimum_control_failure_write_count(1);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-control-failure-sources"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        std::slice::from_ref(&expected),
    );

    let actual = actual_trace_summary("trace.control.failure.sources", 5, 0, 0, 0, 0, 5, 0)
        .with_sync_control_failure_count(1)
        .with_tlb_control_failure_count(1)
        .with_cache_control_failure_count(1)
        .with_htm_control_failure_count(1)
        .with_diagnostic_control_failure_count(1)
        .with_control_failure_invalid_destination_count(4)
        .with_control_failure_write_count(1);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    assert_eq!(result.traffic_trace_replay_summaries(), &[actual]);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_records_control_failure_kind_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.control.kind"))
        .with_minimum_control_failure_count(6)
        .with_minimum_control_failure_invalid_destination_count(1)
        .with_minimum_control_failure_bad_address_count(1)
        .with_minimum_control_failure_read_count(1)
        .with_minimum_control_failure_write_count(1)
        .with_minimum_control_failure_functional_read_count(1)
        .with_minimum_control_failure_functional_write_count(1);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-control-failure-kinds"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_control_failure_invalid_destination_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_control_failure_bad_address_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0].minimum_control_failure_read_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0].minimum_control_failure_write_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_control_failure_functional_read_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_control_failure_functional_write_count(),
        1
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.control.kind"), 6)
        .with_control_failure_count(6)
        .with_control_failure_invalid_destination_count(1)
        .with_control_failure_bad_address_count(1)
        .with_control_failure_read_count(1)
        .with_control_failure_write_count(1)
        .with_control_failure_functional_read_count(1)
        .with_control_failure_functional_write_count(1);
    assert_eq!(actual.control_failure_invalid_destination_count(), 1);
    assert_eq!(actual.control_failure_bad_address_count(), 1);
    assert_eq!(actual.control_failure_read_count(), 1);
    assert_eq!(actual.control_failure_write_count(), 1);
    assert_eq!(actual.control_failure_functional_read_count(), 1);
    assert_eq!(actual.control_failure_functional_write_count(), 1);

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_traffic_trace_replay_summary(actual);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_underreported_control_failure_kind_summary() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.control.kind"))
        .with_minimum_control_failure_count(2)
        .with_minimum_control_failure_invalid_destination_count(2);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("control-failure-kind-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.control.kind"), 2)
        .with_control_failure_count(2)
        .with_control_failure_invalid_destination_count(1);
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
fn workload_manifest_records_control_ack_source_expectations() {
    let expected = expected_trace_summary("trace.control.ack.sources", 2, 0, 0, 0, 2, 0, 0)
        .with_minimum_sync_control_ack_count(1)
        .with_minimum_htm_control_ack_count(1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-control-ack-sources"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        std::slice::from_ref(&expected),
    );

    let actual = actual_trace_summary("trace.control.ack.sources", 2, 0, 0, 0, 2, 0, 0)
        .with_sync_control_ack_count(1)
        .with_htm_control_ack_count(1);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    assert_eq!(result.traffic_trace_replay_summaries(), &[actual]);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_records_trace_write_completion_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.write"))
        .with_minimum_scheduled_count(3)
        .with_minimum_response_delivery_count(1)
        .with_minimum_memory_trace_event_count(3)
        .with_minimum_memory_write_completion_count(1);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-trace-write-completion"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        std::slice::from_ref(&expected),
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.write"), 3)
        .with_response_delivery_count(1)
        .with_memory_trace_event_count(3)
        .with_memory_write_completion_count(1);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    assert_eq!(result.traffic_trace_replay_summaries(), &[actual]);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_records_trace_data_cache_response_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.cache"))
        .with_minimum_scheduled_count(2)
        .with_minimum_response_delivery_count(2)
        .with_minimum_trace_data_cache_response_count(2);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-trace-data-cache-response"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        std::slice::from_ref(&expected),
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.cache"), 2)
        .with_response_delivery_count(2)
        .with_trace_data_cache_response_count(2);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    assert_eq!(result.traffic_trace_replay_summaries(), &[actual]);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_records_trace_error_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
        .with_minimum_scheduled_count(1)
        .with_minimum_memory_failure_count(1)
        .with_minimum_trace_error_count(1)
        .with_minimum_memory_failure_write_count(1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-trace-error"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        std::slice::from_ref(&expected),
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.error"), 1)
        .with_memory_failure_count(1)
        .with_trace_error_count(1)
        .with_memory_failure_write_count(1);
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
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-memory-failure-kinds"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();

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
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("memory-failure-kind-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
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
fn workload_manifest_records_trace_htm_access_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.htm"))
        .with_minimum_scheduled_count(3)
        .with_minimum_response_delivery_count(2)
        .with_minimum_trace_htm_access_count(2);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-trace-htm-access"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        std::slice::from_ref(&expected),
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.htm"), 3)
        .with_response_delivery_count(2)
        .with_trace_htm_access_count(2);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    assert_eq!(result.traffic_trace_replay_summaries(), &[actual]);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_traffic_trace_replay_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-traffic-trace-replay"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let response =
        rem6_workload::WorkloadManifest::builder(id("identity-traffic-trace-replay"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected_trace_summary(
                "trace.a", 1, 1, 3, 0, 0, 0, 0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger =
        rem6_workload::WorkloadManifest::builder(id("identity-traffic-trace-replay"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected_trace_summary(
                "trace.a", 1, 2, 3, 0, 0, 0, 0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let other_route =
        rem6_workload::WorkloadManifest::builder(id("identity-traffic-trace-replay"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected_trace_summary(
                "trace.b", 1, 1, 3, 0, 0, 0, 0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let source_failure =
        rem6_workload::WorkloadManifest::builder(id("identity-traffic-trace-replay"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.a", 1, 1, 3, 0, 0, 0, 0)
                    .with_minimum_cache_control_failure_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let source_ack =
        rem6_workload::WorkloadManifest::builder(id("identity-traffic-trace-replay"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.a", 1, 1, 3, 0, 0, 0, 0)
                    .with_minimum_sync_control_ack_count(1),
            )
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), response.identity());
    assert_ne!(response.identity(), stronger.identity());
    assert_ne!(response.identity(), other_route.identity());
    assert_ne!(response.identity(), source_failure.identity());
    assert_ne!(response.identity(), source_ack.identity());
}

#[test]
fn workload_manifest_identity_changes_with_typed_trace_sideband_expectations() {
    let generic =
        rem6_workload::WorkloadManifest::builder(id("identity-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected_trace_summary(
                "trace.sideband",
                4,
                0,
                0,
                0,
                0,
                0,
                4,
            ))
            .unwrap()
            .build()
            .unwrap();
    let typed =
        rem6_workload::WorkloadManifest::builder(id("identity-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
                    .with_minimum_tlb_sync_event_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let trace_cache_flush =
        rem6_workload::WorkloadManifest::builder(id("identity-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
                    .with_minimum_trace_cache_flush_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let trace_l1_invalidation =
        rem6_workload::WorkloadManifest::builder(id("identity-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
                    .with_minimum_trace_l1_invalidation_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let trace_tlb_sync =
        rem6_workload::WorkloadManifest::builder(id("identity-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
                    .with_minimum_trace_tlb_sync_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let trace_diagnostic =
        rem6_workload::WorkloadManifest::builder(id("identity-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
                    .with_minimum_trace_diagnostic_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let trace_error =
        rem6_workload::WorkloadManifest::builder(id("identity-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
                    .with_minimum_trace_error_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let trace_htm_abort =
        rem6_workload::WorkloadManifest::builder(id("identity-typed-sideband"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
                    .with_minimum_trace_htm_abort_count(1),
            )
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(generic.identity(), typed.identity());
    assert_ne!(generic.identity(), trace_cache_flush.identity());
    assert_ne!(generic.identity(), trace_l1_invalidation.identity());
    assert_ne!(generic.identity(), trace_htm_abort.identity());
    assert_ne!(typed.identity(), trace_cache_flush.identity());
    assert_ne!(typed.identity(), trace_tlb_sync.identity());
    assert_ne!(typed.identity(), trace_htm_abort.identity());
    assert_ne!(trace_tlb_sync.identity(), trace_cache_flush.identity());
    assert_ne!(
        trace_l1_invalidation.identity(),
        trace_cache_flush.identity()
    );
    assert_ne!(trace_l1_invalidation.identity(), trace_tlb_sync.identity());
    assert_ne!(trace_cache_flush.identity(), trace_diagnostic.identity());
    assert_ne!(trace_error.identity(), trace_diagnostic.identity());
    assert_ne!(trace_error.identity(), trace_l1_invalidation.identity());
}

#[test]
fn workload_manifest_identity_changes_with_memory_failure_kind_expectations() {
    let generic =
        rem6_workload::WorkloadManifest::builder(id("identity-memory-failure-kind"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
                    .with_minimum_memory_failure_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let read =
        rem6_workload::WorkloadManifest::builder(id("identity-memory-failure-kind"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
                    .with_minimum_memory_failure_count(1)
                    .with_minimum_memory_failure_read_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let write =
        rem6_workload::WorkloadManifest::builder(id("identity-memory-failure-kind"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
                    .with_minimum_memory_failure_count(1)
                    .with_minimum_memory_failure_write_count(1),
            )
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(generic.identity(), read.identity());
    assert_ne!(read.identity(), write.identity());
}

#[test]
fn workload_manifest_identity_changes_with_control_failure_kind_expectations() {
    let generic =
        rem6_workload::WorkloadManifest::builder(id("identity-control-failure-kind"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.control"))
                    .with_minimum_control_failure_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let invalid =
        rem6_workload::WorkloadManifest::builder(id("identity-control-failure-kind"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.control"))
                    .with_minimum_control_failure_count(1)
                    .with_minimum_control_failure_invalid_destination_count(1),
            )
            .unwrap()
            .build()
            .unwrap();
    let write =
        rem6_workload::WorkloadManifest::builder(id("identity-control-failure-kind"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.control"))
                    .with_minimum_control_failure_count(1)
                    .with_minimum_control_failure_write_count(1),
            )
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(generic.identity(), invalid.identity());
    assert_ne!(invalid.identity(), write.identity());
}

#[test]
fn workload_manifest_identity_changes_with_trace_write_completion_expectations() {
    let generic = rem6_workload::WorkloadManifest::builder(
        id("identity-trace-write-completion"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected_trace_summary(
        "trace.write",
        3,
        1,
        3,
        0,
        0,
        0,
        0,
    ))
    .unwrap()
    .build()
    .unwrap();
    let write_completion = rem6_workload::WorkloadManifest::builder(
        id("identity-trace-write-completion"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(
        expected_trace_summary("trace.write", 3, 1, 3, 0, 0, 0, 0)
            .with_minimum_memory_write_completion_count(1),
    )
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(generic.identity(), write_completion.identity());
}

#[test]
fn workload_manifest_identity_changes_with_trace_response_status_expectations() {
    let generic = rem6_workload::WorkloadManifest::builder(
        id("identity-trace-response-status"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected_trace_summary(
        "trace.response",
        3,
        1,
        3,
        0,
        0,
        0,
        0,
    ))
    .unwrap()
    .build()
    .unwrap();
    let status = rem6_workload::WorkloadManifest::builder(
        id("identity-trace-response-status"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(
        expected_trace_summary("trace.response", 3, 1, 3, 0, 0, 0, 0)
            .with_minimum_trace_completed_response_count(1),
    )
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(generic.identity(), status.identity());
}

#[test]
fn workload_manifest_identity_changes_with_trace_data_cache_response_expectations() {
    let generic = rem6_workload::WorkloadManifest::builder(
        id("identity-trace-data-cache-response"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected_trace_summary(
        "trace.cache",
        2,
        2,
        4,
        0,
        0,
        0,
        0,
    ))
    .unwrap()
    .build()
    .unwrap();
    let cache_response = rem6_workload::WorkloadManifest::builder(
        id("identity-trace-data-cache-response"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(
        expected_trace_summary("trace.cache", 2, 2, 4, 0, 0, 0, 0)
            .with_minimum_trace_data_cache_response_count(2),
    )
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(generic.identity(), cache_response.identity());
}

#[test]
fn workload_manifest_identity_changes_with_trace_htm_access_expectations() {
    let generic =
        rem6_workload::WorkloadManifest::builder(id("identity-trace-htm-access"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected_trace_summary(
                "trace.htm",
                3,
                2,
                3,
                0,
                0,
                0,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let htm_access =
        rem6_workload::WorkloadManifest::builder(id("identity-trace-htm-access"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(
                expected_trace_summary("trace.htm", 3, 2, 3, 0, 0, 0, 0)
                    .with_minimum_trace_htm_access_count(2),
            )
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(generic.identity(), htm_access.identity());
}

#[test]
fn traffic_trace_replay_identity_domain_stays_distinct_from_checkpoint_expectations() {
    let checkpoint =
        rem6_workload::WorkloadManifest::builder(id("traffic-trace-domain"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_checkpoint_manifest_summary(
                WorkloadExpectedCheckpointManifestSummary::new("trace.a", 1, 1, 3),
            )
            .unwrap()
            .build()
            .unwrap();
    let traffic =
        rem6_workload::WorkloadManifest::builder(id("traffic-trace-domain"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected_trace_summary(
                "trace.a", 1, 1, 3, 0, 0, 0, 0,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(checkpoint.identity(), traffic.identity());
}

#[test]
fn workload_manifest_rejects_duplicate_traffic_trace_replay_expectations() {
    let error = rem6_workload::WorkloadManifest::builder(
        id("duplicate-traffic-trace-replay"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected_trace_summary(
        "trace.a", 1, 1, 3, 0, 0, 0, 0,
    ))
    .unwrap()
    .add_expected_traffic_trace_replay_summary(expected_trace_summary(
        "trace.a", 2, 1, 3, 0, 0, 0, 0,
    ))
    .unwrap_err();

    assert_eq!(
        error,
        WorkloadError::DuplicateExpectedTrafficTraceReplaySummary {
            route: route_id("trace.a"),
        },
    );
}

#[test]
fn workload_replay_plan_rejects_missing_or_underreported_traffic_trace_replay_summary() {
    let expected = expected_trace_summary("trace.a", 1, 2, 3, 1, 1, 0, 1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("traffic-trace-replay-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
            WorkloadTrafficTraceReplaySummaryExpectationError::Missing(expected.clone()),
        )),
    );

    let actual = actual_trace_summary("trace.a", 1, 1, 3, 0, 1, 0, 0);
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
fn workload_replay_plan_rejects_underreported_typed_sideband_summary() {
    let expected = expected_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4)
        .with_minimum_tlb_sync_event_count(1)
        .with_minimum_trace_tlb_sync_count(1)
        .with_minimum_cache_flush_event_count(1)
        .with_minimum_trace_cache_flush_count(1)
        .with_minimum_trace_l1_invalidation_count(1)
        .with_minimum_trace_diagnostic_count(1)
        .with_minimum_trace_htm_abort_count(1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("typed-sideband-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual =
        actual_trace_summary("trace.sideband", 4, 0, 0, 0, 0, 0, 4).with_tlb_sync_event_count(1);
    let underreported = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    let error = plan.verify_result(&underreported).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
            WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum { expected, actual },
        )),
    );
    assert_eq!(
        error.to_string(),
        "traffic trace replay summary for route trace.sideband has scheduled 4/4, responses 0/0, trace completed responses 0/0, trace retry responses 0/0, trace store-conditional failed responses 0/0, memory trace events 0/0, memory write completions 0/0, trace data-cache responses 0/0, trace data-cache errors 0/0, memory failures 0/0, memory failure invalid destinations 0/0, memory failure bad addresses 0/0, memory failure reads 0/0, memory failure writes 0/0, memory failure functional reads 0/0, memory failure functional writes 0/0, trace errors 0/0, trace htm accesses 0/0, control acks 0/0, sync control acks 0/0, htm control acks 0/0, control failures 0/0, control failure invalid destinations 0/0, control failure bad addresses 0/0, control failure reads 0/0, control failure writes 0/0, control failure functional reads 0/0, control failure functional writes 0/0, sync control failures 0/0, tlb control failures 0/0, cache control failures 0/0, htm control failures 0/0, diagnostic control failures 0/0, sideband events 4/4, tlb sync events 1/1, trace tlb syncs 0/1, cache flush events 0/1, trace cache flushes 0/1, trace l1 invalidations 0/1, diagnostic print events 0/0, trace diagnostics 0/1, htm abort events 0/0, trace htm aborts 0/1",
    );
}

#[test]
fn workload_replay_plan_rejects_underreported_control_failure_source_summary() {
    let expected = expected_trace_summary("trace.control.failure.sources", 5, 0, 0, 0, 0, 5, 0)
        .with_minimum_cache_control_failure_count(2);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("control-failure-source-mismatch"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = actual_trace_summary("trace.control.failure.sources", 5, 0, 0, 0, 0, 5, 0)
        .with_cache_control_failure_count(1);
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
fn workload_replay_plan_rejects_underreported_trace_write_completion_summary() {
    let expected = expected_trace_summary("trace.write", 3, 1, 3, 0, 0, 0, 0)
        .with_minimum_memory_write_completion_count(1);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("trace-write-completion-mismatch"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = actual_trace_summary("trace.write", 3, 1, 3, 0, 0, 0, 0);
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
fn workload_replay_plan_rejects_underreported_control_ack_source_summary() {
    let expected = expected_trace_summary("trace.control.ack.sources", 2, 0, 0, 0, 2, 0, 0)
        .with_minimum_htm_control_ack_count(2);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("control-ack-source-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = actual_trace_summary("trace.control.ack.sources", 2, 0, 0, 0, 2, 0, 0)
        .with_htm_control_ack_count(1);
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
fn workload_replay_plan_rejects_underreported_trace_data_cache_response_summary() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.cache"))
        .with_minimum_response_delivery_count(2)
        .with_minimum_trace_data_cache_response_count(2);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("trace-data-cache-response-mismatch"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.cache"), 2)
        .with_response_delivery_count(2)
        .with_trace_data_cache_response_count(1);
    let underreported = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    let error = plan.verify_result(&underreported).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
            WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum { expected, actual },
        )),
    );
    assert_eq!(
        error.to_string(),
        "traffic trace replay summary for route trace.cache has scheduled 2/0, responses 2/2, trace completed responses 0/0, trace retry responses 0/0, trace store-conditional failed responses 0/0, memory trace events 0/0, memory write completions 0/0, trace data-cache responses 1/2, trace data-cache errors 0/0, memory failures 0/0, memory failure invalid destinations 0/0, memory failure bad addresses 0/0, memory failure reads 0/0, memory failure writes 0/0, memory failure functional reads 0/0, memory failure functional writes 0/0, trace errors 0/0, trace htm accesses 0/0, control acks 0/0, sync control acks 0/0, htm control acks 0/0, control failures 0/0, control failure invalid destinations 0/0, control failure bad addresses 0/0, control failure reads 0/0, control failure writes 0/0, control failure functional reads 0/0, control failure functional writes 0/0, sync control failures 0/0, tlb control failures 0/0, cache control failures 0/0, htm control failures 0/0, diagnostic control failures 0/0, sideband events 0/0, tlb sync events 0/0, trace tlb syncs 0/0, cache flush events 0/0, trace cache flushes 0/0, trace l1 invalidations 0/0, diagnostic print events 0/0, trace diagnostics 0/0, htm abort events 0/0, trace htm aborts 0/0",
    );
}

#[test]
fn workload_replay_plan_rejects_underreported_trace_data_cache_error_summary() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.cache.error"))
        .with_minimum_trace_data_cache_error_count(2);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("trace-data-cache-error-mismatch"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_traffic_trace_replay_summary(expected.clone())
    .unwrap()
    .build()
    .unwrap();
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
    assert_eq!(
        error.to_string(),
        "traffic trace replay summary for route trace.cache.error has scheduled 1/0, responses 0/0, trace completed responses 0/0, trace retry responses 0/0, trace store-conditional failed responses 0/0, memory trace events 0/0, memory write completions 0/0, trace data-cache responses 0/0, trace data-cache errors 1/2, memory failures 0/0, memory failure invalid destinations 0/0, memory failure bad addresses 0/0, memory failure reads 0/0, memory failure writes 0/0, memory failure functional reads 0/0, memory failure functional writes 0/0, trace errors 0/0, trace htm accesses 0/0, control acks 0/0, sync control acks 0/0, htm control acks 0/0, control failures 0/0, control failure invalid destinations 0/0, control failure bad addresses 0/0, control failure reads 0/0, control failure writes 0/0, control failure functional reads 0/0, control failure functional writes 0/0, sync control failures 0/0, tlb control failures 0/0, cache control failures 0/0, htm control failures 0/0, diagnostic control failures 0/0, sideband events 0/0, tlb sync events 0/0, trace tlb syncs 0/0, cache flush events 0/0, trace cache flushes 0/0, trace l1 invalidations 0/0, diagnostic print events 0/0, trace diagnostics 0/0, htm abort events 0/0, trace htm aborts 0/0",
    );
}

#[test]
fn workload_replay_plan_rejects_underreported_trace_error_summary() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
        .with_minimum_memory_failure_count(1)
        .with_minimum_trace_error_count(1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("trace-error-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.error"), 1)
        .with_memory_failure_count(1);
    let underreported = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    let error = plan.verify_result(&underreported).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
            WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum { expected, actual },
        )),
    );
    assert_eq!(
        error.to_string(),
        "traffic trace replay summary for route trace.error has scheduled 1/0, responses 0/0, trace completed responses 0/0, trace retry responses 0/0, trace store-conditional failed responses 0/0, memory trace events 0/0, memory write completions 0/0, trace data-cache responses 0/0, trace data-cache errors 0/0, memory failures 1/1, memory failure invalid destinations 0/0, memory failure bad addresses 0/0, memory failure reads 0/0, memory failure writes 0/0, memory failure functional reads 0/0, memory failure functional writes 0/0, trace errors 0/1, trace htm accesses 0/0, control acks 0/0, sync control acks 0/0, htm control acks 0/0, control failures 0/0, control failure invalid destinations 0/0, control failure bad addresses 0/0, control failure reads 0/0, control failure writes 0/0, control failure functional reads 0/0, control failure functional writes 0/0, sync control failures 0/0, tlb control failures 0/0, cache control failures 0/0, htm control failures 0/0, diagnostic control failures 0/0, sideband events 0/0, tlb sync events 0/0, trace tlb syncs 0/0, cache flush events 0/0, trace cache flushes 0/0, trace l1 invalidations 0/0, diagnostic print events 0/0, trace diagnostics 0/0, htm abort events 0/0, trace htm aborts 0/0",
    );
}

#[test]
fn workload_replay_plan_rejects_underreported_trace_htm_access_summary() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.htm"))
        .with_minimum_response_delivery_count(2)
        .with_minimum_trace_htm_access_count(2);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("trace-htm-access-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_traffic_trace_replay_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.htm"), 3)
        .with_response_delivery_count(2);
    let underreported = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    let error = plan.verify_result(&underreported).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
            WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum { expected, actual },
        )),
    );
    assert_eq!(
        error.to_string(),
        "traffic trace replay summary for route trace.htm has scheduled 3/0, responses 2/2, trace completed responses 0/0, trace retry responses 0/0, trace store-conditional failed responses 0/0, memory trace events 0/0, memory write completions 0/0, trace data-cache responses 0/0, trace data-cache errors 0/0, memory failures 0/0, memory failure invalid destinations 0/0, memory failure bad addresses 0/0, memory failure reads 0/0, memory failure writes 0/0, memory failure functional reads 0/0, memory failure functional writes 0/0, trace errors 0/0, trace htm accesses 0/2, control acks 0/0, sync control acks 0/0, htm control acks 0/0, control failures 0/0, control failure invalid destinations 0/0, control failure bad addresses 0/0, control failure reads 0/0, control failure writes 0/0, control failure functional reads 0/0, control failure functional writes 0/0, sync control failures 0/0, tlb control failures 0/0, cache control failures 0/0, htm control failures 0/0, diagnostic control failures 0/0, sideband events 0/0, tlb sync events 0/0, trace tlb syncs 0/0, cache flush events 0/0, trace cache flushes 0/0, trace l1 invalidations 0/0, diagnostic print events 0/0, trace diagnostics 0/0, htm abort events 0/0, trace htm aborts 0/0",
    );
}

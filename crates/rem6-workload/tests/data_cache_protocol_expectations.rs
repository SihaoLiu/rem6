use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadError,
    WorkloadExpectedDataCacheProtocolRunCount, WorkloadExpectedDataCacheRunAttribution, WorkloadId,
    WorkloadParallelExecutionSummary, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
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

fn replay_plan() -> WorkloadReplayPlan {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("data-cache-protocol-count"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_protocol(
    protocol: WorkloadDataCacheProtocol,
    minimum_run_count: usize,
) -> WorkloadExpectedDataCacheProtocolRunCount {
    WorkloadExpectedDataCacheProtocolRunCount::new(protocol, minimum_run_count).unwrap()
}

fn expected_attribution(
    minimum_attributed_run_count: usize,
    maximum_unattributed_run_count: usize,
) -> WorkloadExpectedDataCacheRunAttribution {
    WorkloadExpectedDataCacheRunAttribution::new(
        minimum_attributed_run_count,
        maximum_unattributed_run_count,
    )
}

#[test]
fn workload_manifest_records_data_cache_protocol_run_expectations() {
    let msi = expected_protocol(WorkloadDataCacheProtocol::Msi, 2);
    let chi = expected_protocol(WorkloadDataCacheProtocol::Chi, 1);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-data-cache-protocol"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_data_cache_protocol_run_count(chi)
            .unwrap()
            .add_expected_data_cache_protocol_run_count(msi)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_data_cache_protocol_run_counts(),
        &[msi, chi],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_data_cache_protocol_run_counts(),
        manifest.expected_data_cache_protocol_run_counts(),
    );

    let summary = WorkloadParallelExecutionSummary::default().with_data_cache_protocol_counts([
        WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Msi, 2),
        WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Chi, 1),
    ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_records_data_cache_run_attribution_expectation() {
    let expected = expected_attribution(2, 0);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-data-cache-attribution"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_data_cache_run_attribution(expected)
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(
        manifest.expected_data_cache_run_attribution(),
        Some(&expected),
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_data_cache_run_attribution(),
        manifest.expected_data_cache_run_attribution(),
    );

    let summary = WorkloadParallelExecutionSummary::default().with_data_cache_run_attribution(2, 0);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_data_cache_protocol_run_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-data-cache-protocol"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let msi =
        rem6_workload::WorkloadManifest::builder(id("identity-data-cache-protocol"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_data_cache_protocol_run_count(expected_protocol(
                WorkloadDataCacheProtocol::Msi,
                1,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger =
        rem6_workload::WorkloadManifest::builder(id("identity-data-cache-protocol"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_data_cache_protocol_run_count(expected_protocol(
                WorkloadDataCacheProtocol::Msi,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let chi =
        rem6_workload::WorkloadManifest::builder(id("identity-data-cache-protocol"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_data_cache_protocol_run_count(expected_protocol(
                WorkloadDataCacheProtocol::Chi,
                1,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), msi.identity());
    assert_ne!(msi.identity(), stronger.identity());
    assert_ne!(msi.identity(), chi.identity());
}

#[test]
fn workload_manifest_identity_changes_with_data_cache_run_attribution_expectation() {
    let base = rem6_workload::WorkloadManifest::builder(
        id("identity-data-cache-attribution"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let attributed = rem6_workload::WorkloadManifest::builder(
        id("identity-data-cache-attribution"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_data_cache_run_attribution(expected_attribution(1, 0))
    .unwrap()
    .build()
    .unwrap();
    let stronger = rem6_workload::WorkloadManifest::builder(
        id("identity-data-cache-attribution"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_data_cache_run_attribution(expected_attribution(2, 0))
    .unwrap()
    .build()
    .unwrap();
    let tolerant = rem6_workload::WorkloadManifest::builder(
        id("identity-data-cache-attribution"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_data_cache_run_attribution(expected_attribution(1, 1))
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(base.identity(), attributed.identity());
    assert_ne!(attributed.identity(), stronger.identity());
    assert_ne!(attributed.identity(), tolerant.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_data_cache_protocol_runs() {
    let plan = replay_plan()
        .add_expected_data_cache_protocol_run_count(expected_protocol(
            WorkloadDataCacheProtocol::Chi,
            2,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingDataCacheProtocolSummary {
            protocol: WorkloadDataCacheProtocol::Chi,
            minimum_run_count: 2,
        },
    );

    let underactive_summary =
        WorkloadParallelExecutionSummary::default().with_data_cache_protocol_counts([
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Chi, 1),
        ]);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedDataCacheProtocolRunCountBelowMinimum {
            protocol: WorkloadDataCacheProtocol::Chi,
            minimum_run_count: 2,
            actual_run_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_missing_or_mismatched_data_cache_run_attribution() {
    let plan = replay_plan()
        .add_expected_data_cache_run_attribution(expected_attribution(2, 0))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingDataCacheRunAttributionSummary {
            minimum_attributed_run_count: 2,
            maximum_unattributed_run_count: 0,
        },
    );

    let underattributed_summary =
        WorkloadParallelExecutionSummary::default().with_data_cache_run_attribution(1, 0);
    let underattributed = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underattributed_summary);
    assert_eq!(
        plan.verify_result(&underattributed).unwrap_err(),
        WorkloadError::ExpectedDataCacheRunAttributionBelowMinimum {
            minimum_attributed_run_count: 2,
            actual_attributed_run_count: 1,
        },
    );

    let unattributed_summary =
        WorkloadParallelExecutionSummary::default().with_data_cache_run_attribution(2, 1);
    let unattributed = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(unattributed_summary);
    assert_eq!(
        plan.verify_result(&unattributed).unwrap_err(),
        WorkloadError::ExpectedDataCacheRunAttributionAboveMaximum {
            maximum_unattributed_run_count: 0,
            actual_unattributed_run_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_data_cache_protocol_runs() {
    let zero = WorkloadExpectedDataCacheProtocolRunCount::new(WorkloadDataCacheProtocol::Msi, 0)
        .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedDataCacheProtocolRunCount {
            protocol: WorkloadDataCacheProtocol::Msi,
        },
    );

    let duplicate = replay_plan()
        .add_expected_data_cache_protocol_run_count(expected_protocol(
            WorkloadDataCacheProtocol::Mesi,
            1,
        ))
        .unwrap()
        .add_expected_data_cache_protocol_run_count(expected_protocol(
            WorkloadDataCacheProtocol::Mesi,
            2,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedDataCacheProtocolRunCount {
            protocol: WorkloadDataCacheProtocol::Mesi,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_data_cache_run_attribution() {
    let duplicate = replay_plan()
        .add_expected_data_cache_run_attribution(expected_attribution(1, 0))
        .unwrap()
        .add_expected_data_cache_run_attribution(expected_attribution(2, 0))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedDataCacheRunAttribution
    );
}

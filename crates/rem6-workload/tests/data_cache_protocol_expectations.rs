use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadError,
    WorkloadExpectedDataCacheProtocolRunCount, WorkloadId, WorkloadParallelExecutionSummary,
    WorkloadReplayPlan, WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
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

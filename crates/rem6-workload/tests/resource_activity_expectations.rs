use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedResourceActivity, WorkloadId, WorkloadParallelExecutionSummary,
    WorkloadReplayPlan, WorkloadResource, WorkloadResourceActivityScope, WorkloadResourceId,
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
    let manifest = rem6_workload::WorkloadManifest::builder(id("resource-activity"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_activity(
    scope: WorkloadResourceActivityScope,
    minimum_operation_count: usize,
    minimum_active_resource_count: usize,
) -> WorkloadExpectedResourceActivity {
    WorkloadExpectedResourceActivity::new(
        scope,
        minimum_operation_count,
        minimum_active_resource_count,
    )
    .unwrap()
}

#[test]
fn workload_manifest_records_resource_activity_expectations() {
    let fabric = expected_activity(WorkloadResourceActivityScope::Fabric, 7, 2);
    let dram = expected_activity(WorkloadResourceActivityScope::Dram, 5, 1);
    let resource = expected_activity(WorkloadResourceActivityScope::Resource, 12, 3);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-resource-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_resource_activity(resource)
            .unwrap()
            .add_expected_resource_activity(dram)
            .unwrap()
            .add_expected_resource_activity(fabric)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_resource_activity(),
        &[fabric, dram, resource],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_resource_activity(),
        manifest.expected_resource_activity(),
    );
    assert_eq!(WorkloadResourceActivityScope::Fabric.as_str(), "fabric");
    assert_eq!(WorkloadResourceActivityScope::Dram.as_str(), "dram");
    assert_eq!(WorkloadResourceActivityScope::Resource.as_str(), "resource");

    let summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_activity(2, 7, 224, 31, 13, 8, 1)
        .with_dram_activity(1, 2, 3, 5, 4, 1, 2, 3, 11, 1, 83, 21);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_resource_activity() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-resource-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let fabric =
        rem6_workload::WorkloadManifest::builder(id("identity-resource-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_resource_activity(expected_activity(
                WorkloadResourceActivityScope::Fabric,
                1,
                1,
            ))
            .unwrap()
            .build()
            .unwrap();
    let dram =
        rem6_workload::WorkloadManifest::builder(id("identity-resource-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_resource_activity(expected_activity(
                WorkloadResourceActivityScope::Dram,
                1,
                1,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), fabric.identity());
    assert_ne!(fabric.identity(), dram.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_resource_activity() {
    let plan = replay_plan()
        .add_expected_resource_activity(expected_activity(
            WorkloadResourceActivityScope::Fabric,
            4,
            2,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingResourceActivitySummary {
            scope: WorkloadResourceActivityScope::Fabric,
            minimum_operation_count: 4,
            minimum_active_resource_count: 2,
        },
    );

    let underactive_summary =
        WorkloadParallelExecutionSummary::default().with_fabric_activity(1, 3, 96, 12, 4, 3, 0);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedResourceActivityBelowMinimum {
            scope: WorkloadResourceActivityScope::Fabric,
            minimum_operation_count: 4,
            actual_operation_count: 3,
            minimum_active_resource_count: 2,
            actual_active_resource_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_resource_activity() {
    let zero = WorkloadExpectedResourceActivity::new(WorkloadResourceActivityScope::Resource, 0, 0)
        .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedResourceActivity {
            scope: WorkloadResourceActivityScope::Resource,
        },
    );

    let duplicate = replay_plan()
        .add_expected_resource_activity(expected_activity(
            WorkloadResourceActivityScope::Dram,
            1,
            1,
        ))
        .unwrap()
        .add_expected_resource_activity(expected_activity(
            WorkloadResourceActivityScope::Dram,
            2,
            1,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedResourceActivity {
            scope: WorkloadResourceActivityScope::Dram,
        },
    );
}

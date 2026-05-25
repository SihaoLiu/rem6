use rem6_boot::BootImage;
use rem6_kernel::{ParallelPartitionActivity, ParallelRemoteFlowRecord, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelPartitionUse, WorkloadId,
    WorkloadParallelExecutionSummary, WorkloadParallelRemoteFlowScope, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
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

fn expected_partitions(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_active_partitions: usize,
) -> WorkloadExpectedParallelPartitionUse {
    WorkloadExpectedParallelPartitionUse::new(scope, minimum_active_partitions).unwrap()
}

#[test]
fn workload_manifest_records_parallel_partition_expectations() {
    let scheduler_partitions = expected_partitions(WorkloadParallelRemoteFlowScope::Scheduler, 2);
    let data_cache_partitions =
        expected_partitions(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3);
    let full_system_partitions =
        expected_partitions(WorkloadParallelRemoteFlowScope::FullSystem, 4);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(full_system_partitions)
            .unwrap()
            .add_expected_parallel_partition_use(data_cache_partitions)
            .unwrap()
            .add_expected_parallel_partition_use(scheduler_partitions)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_partition_use(),
        &[
            scheduler_partitions,
            data_cache_partitions,
            full_system_partitions,
        ],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_partition_use(),
        manifest.expected_parallel_partition_use(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_partitions(2, 2)
        .with_data_cache_parallel_counts(1, 1, 2, 1, 3)
        .with_data_cache_parallel_partitions(3)
        .with_full_system_parallel_partitions(4);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_partition_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::Scheduler,
                3,
            ))
            .unwrap()
            .build()
            .unwrap();
    let full_system =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::FullSystem,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger_scheduler.identity());
    assert_ne!(scheduler.identity(), full_system.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underused_parallel_partitions() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-partitions-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelPartitionSummary {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_active_partitions: 2,
        },
    );

    let single_partition_summary =
        WorkloadParallelExecutionSummary::default().with_scheduler_partitions(1, 1);
    let single_partition_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(single_partition_summary);
    assert_eq!(
        plan.verify_result(&single_partition_result).unwrap_err(),
        WorkloadError::ExpectedParallelPartitionCountBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_active_partitions: 2,
            actual_active_partitions: 1,
        },
    );
}

#[test]
fn workload_replay_plan_derives_full_system_partition_use_from_activity() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-from-activity"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::FullSystem,
            3,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([
            (
                PartitionId::new(0),
                ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 1),
            ),
            (
                PartitionId::new(1),
                ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 1),
            ),
        ])
        .with_data_cache_parallel_scheduler_partition_activities([
            (
                PartitionId::new(1),
                ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 1),
            ),
            (
                PartitionId::new(2),
                ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 1),
            ),
        ]);

    assert_eq!(
        summary.active_full_system_parallel_scheduler_partition_count(),
        3
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_derives_active_partitions_from_remote_flows() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-from-flows"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            3,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::FullSystem,
            5,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 5, 3, 17),
            ParallelRemoteFlowRecord::new(PartitionId::new(2), PartitionId::new(1), 3, 5, 13),
        ])
        .with_data_cache_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(3),
            PartitionId::new(4),
            4,
            7,
            11,
        )]);

    assert_eq!(summary.active_scheduler_partition_count(), 3);
    assert_eq!(
        summary.active_data_cache_parallel_scheduler_partition_count(),
        2,
    );
    assert_eq!(
        summary.active_full_system_parallel_scheduler_partition_count(),
        5,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_parallel_partition_expectations() {
    let zero =
        WorkloadExpectedParallelPartitionUse::new(WorkloadParallelRemoteFlowScope::FullSystem, 0)
            .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedParallelPartitionCount {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
        },
    );

    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-partitions-duplicate"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::FullSystem,
                2,
            ))
            .unwrap()
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::FullSystem,
                3,
            ))
            .unwrap_err();
    assert_eq!(
        manifest,
        WorkloadError::DuplicateExpectedParallelPartitionUse {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
        },
    );
}

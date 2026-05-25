use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchActivity, WorkloadId,
    WorkloadParallelBatchPartitionSet, WorkloadParallelBatchWorkerCount,
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

fn replay_plan() -> WorkloadReplayPlan {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-batch-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_activity(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_worker_count: usize,
    minimum_batch_count: usize,
) -> WorkloadExpectedParallelBatchActivity {
    WorkloadExpectedParallelBatchActivity::new(scope, minimum_worker_count, minimum_batch_count)
        .unwrap()
}

fn partition(index: u32) -> PartitionId {
    PartitionId::new(index)
}

#[test]
fn workload_manifest_records_parallel_batch_activity_expectations() {
    let scheduler_activity = expected_activity(WorkloadParallelRemoteFlowScope::Scheduler, 2, 3);
    let data_cache_activity =
        expected_activity(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3, 2);
    let full_system_activity = expected_activity(WorkloadParallelRemoteFlowScope::FullSystem, 2, 5);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-batch-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_activity(full_system_activity)
            .unwrap()
            .add_expected_parallel_batch_activity(data_cache_activity)
            .unwrap()
            .add_expected_parallel_batch_activity(scheduler_activity)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_batch_activity(),
        &[
            scheduler_activity,
            data_cache_activity,
            full_system_activity
        ],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_batch_activity(),
        manifest.expected_parallel_batch_activity(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(1, 4),
            WorkloadParallelBatchWorkerCount::new(2, 2),
            WorkloadParallelBatchWorkerCount::new(3, 1),
        ])
        .with_data_cache_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(3, 2),
        ]);
    assert_eq!(summary.parallel_scheduler_batch_count_at_or_above(2), 3);
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_at_or_above(3),
        2,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_at_or_above(2),
        5,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_batch_activity() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_activity(expected_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                3,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_activity(expected_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                4,
            ))
            .unwrap()
            .build()
            .unwrap();
    let wider_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_activity(expected_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                3,
                3,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger_scheduler.identity());
    assert_ne!(scheduler.identity(), wider_scheduler.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_parallel_batches() {
    let plan = replay_plan()
        .add_expected_parallel_batch_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            3,
            3,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchActivitySummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_worker_count: 3,
            minimum_batch_count: 3,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_worker_counts([WorkloadParallelBatchWorkerCount::new(3, 1)])
        .with_data_cache_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(2, 4),
        ]);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedParallelBatchActivityBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_worker_count: 3,
            minimum_batch_count: 3,
            actual_batch_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_derives_batch_activity_from_partition_sets() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-batch-activity-from-partitions"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_batch_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            6,
        ))
        .unwrap()
        .add_expected_parallel_batch_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            3,
            4,
        ))
        .unwrap()
        .add_expected_parallel_batch_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            2,
            11,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(1)], 4),
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(2), partition(4)], 2),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new(
                [partition(10), partition(11), partition(12)],
                3,
            ),
            WorkloadParallelBatchPartitionSet::new(
                [partition(10), partition(13), partition(14), partition(15)],
                1,
            ),
            WorkloadParallelBatchPartitionSet::new([partition(12), partition(15)], 1),
        ]);

    assert_eq!(summary.parallel_scheduler_batch_count_at_or_above(2), 6);
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_at_or_above(3),
        4,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_at_or_above(2),
        11,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_batch_activity() {
    let invalid_worker_count = WorkloadExpectedParallelBatchActivity::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        1,
        3,
    )
    .unwrap_err();
    assert_eq!(
        invalid_worker_count,
        WorkloadError::InvalidExpectedParallelBatchWorkerCount {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_worker_count: 1,
        },
    );

    let zero_batch_count = WorkloadExpectedParallelBatchActivity::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        2,
        0,
    )
    .unwrap_err();
    assert_eq!(
        zero_batch_count,
        WorkloadError::ZeroExpectedParallelBatchCount {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_worker_count: 2,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_batch_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            3,
        ))
        .unwrap()
        .add_expected_parallel_batch_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            4,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelBatchActivity {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_worker_count: 2,
        },
    );
}

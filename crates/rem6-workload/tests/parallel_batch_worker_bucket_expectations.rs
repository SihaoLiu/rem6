use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchWorkerBucket, WorkloadId,
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
        rem6_workload::WorkloadManifest::builder(id("parallel-batch-worker-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_bucket(
    scope: WorkloadParallelRemoteFlowScope,
    worker_count: usize,
    minimum_batch_count: usize,
) -> WorkloadExpectedParallelBatchWorkerBucket {
    WorkloadExpectedParallelBatchWorkerBucket::new(scope, worker_count, minimum_batch_count)
        .unwrap()
}

fn partition(index: u32) -> PartitionId {
    PartitionId::new(index)
}

#[test]
fn workload_manifest_records_parallel_batch_worker_bucket_expectations() {
    let scheduler_bucket = expected_bucket(WorkloadParallelRemoteFlowScope::Scheduler, 2, 3);
    let data_cache_bucket =
        expected_bucket(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3, 2);
    let full_system_bucket = expected_bucket(WorkloadParallelRemoteFlowScope::FullSystem, 2, 5);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-worker-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_bucket(full_system_bucket)
            .unwrap()
            .add_expected_parallel_batch_worker_bucket(data_cache_bucket)
            .unwrap()
            .add_expected_parallel_batch_worker_bucket(scheduler_bucket)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_batch_worker_buckets(),
        &[scheduler_bucket, data_cache_bucket, full_system_bucket],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_batch_worker_buckets(),
        manifest.expected_parallel_batch_worker_buckets(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(2, 3),
            WorkloadParallelBatchWorkerCount::new(3, 1),
        ])
        .with_data_cache_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(2, 2),
            WorkloadParallelBatchWorkerCount::new(3, 2),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_batch_worker_buckets() {
    let base = rem6_workload::WorkloadManifest::builder(id("identity-worker-bucket"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_bucket(expected_bucket(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                3,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_bucket(expected_bucket(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                4,
            ))
            .unwrap()
            .build()
            .unwrap();
    let wider_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_bucket(expected_bucket(
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
fn workload_replay_plan_rejects_missing_or_underfilled_parallel_batch_worker_buckets() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_bucket(expected_bucket(
            WorkloadParallelRemoteFlowScope::FullSystem,
            3,
            3,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchWorkerBucketSummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            worker_count: 3,
            minimum_batch_count: 3,
        },
    );

    let underfilled_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_worker_counts([WorkloadParallelBatchWorkerCount::new(3, 1)])
        .with_data_cache_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(2, 4),
        ]);
    let underfilled = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underfilled_summary);
    assert_eq!(
        plan.verify_result(&underfilled).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerBucketBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            worker_count: 3,
            minimum_batch_count: 3,
            actual_batch_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_derives_parallel_batch_worker_buckets_from_partition_evidence() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_bucket(expected_bucket(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            4,
        ))
        .unwrap()
        .add_expected_parallel_batch_worker_bucket(expected_bucket(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            3,
            3,
        ))
        .unwrap()
        .add_expected_parallel_batch_worker_bucket(expected_bucket(
            WorkloadParallelRemoteFlowScope::FullSystem,
            2,
            6,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(1)], 4),
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(2), partition(4)], 2),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_streak_sequence([
            WorkloadParallelBatchPartitionSet::new([partition(10), partition(11)], 2),
            WorkloadParallelBatchPartitionSet::new(
                [partition(10), partition(12), partition(13)],
                3,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_batch_worker_buckets() {
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerBucket::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            1,
            3,
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchWorkerBucket {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            worker_count: 1,
        },
    );
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerBucket::new(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
            0,
        )
        .unwrap_err(),
        WorkloadError::ZeroExpectedParallelBatchWorkerBucket {
            scope: WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            worker_count: 2,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_batch_worker_bucket(expected_bucket(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            3,
        ))
        .unwrap()
        .add_expected_parallel_batch_worker_bucket(expected_bucket(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            4,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelBatchWorkerBucket {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            worker_count: 2,
        },
    );
}

use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchWorkerBucket,
    WorkloadExpectedParallelBatchWorkerTickActivity, WorkloadExpectedParallelBatchWorkerTickBucket,
    WorkloadExpectedParallelBatchWorkerTickStreak, WorkloadExpectedParallelBatchWorkerTicks,
    WorkloadId, WorkloadParallelBatchPartitionSet, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord, WorkloadParallelBatchWorkerCount,
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

fn expected_tick_bucket(
    scope: WorkloadParallelRemoteFlowScope,
    worker_count: usize,
    minimum_ticks: u64,
) -> WorkloadExpectedParallelBatchWorkerTickBucket {
    WorkloadExpectedParallelBatchWorkerTickBucket::new(scope, worker_count, minimum_ticks).unwrap()
}

fn expected_tick_activity(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_worker_count: usize,
    minimum_ticks: u64,
) -> WorkloadExpectedParallelBatchWorkerTickActivity {
    WorkloadExpectedParallelBatchWorkerTickActivity::new(scope, minimum_worker_count, minimum_ticks)
        .unwrap()
}

fn expected_tick_streak(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_worker_count: usize,
    minimum_consecutive_ticks: u64,
) -> WorkloadExpectedParallelBatchWorkerTickStreak {
    WorkloadExpectedParallelBatchWorkerTickStreak::new(
        scope,
        minimum_worker_count,
        minimum_consecutive_ticks,
    )
    .unwrap()
}

fn expected_worker_ticks(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_worker_ticks: u64,
) -> WorkloadExpectedParallelBatchWorkerTicks {
    WorkloadExpectedParallelBatchWorkerTicks::new(scope, minimum_worker_ticks).unwrap()
}

fn timeline_record(
    scope: WorkloadParallelBatchScope,
    start_tick: u64,
    horizon: u64,
    partitions: impl IntoIterator<Item = PartitionId>,
    worker_count: usize,
) -> WorkloadParallelBatchTimelineRecord {
    WorkloadParallelBatchTimelineRecord::new(scope, start_tick, horizon, partitions, worker_count)
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

#[test]
fn workload_manifest_records_parallel_batch_worker_tick_bucket_expectations() {
    let scheduler_bucket = expected_tick_bucket(WorkloadParallelRemoteFlowScope::Scheduler, 2, 8);
    let data_cache_bucket =
        expected_tick_bucket(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3, 4);
    let full_system_bucket =
        expected_tick_bucket(WorkloadParallelRemoteFlowScope::FullSystem, 2, 12);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-worker-tick-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_bucket(full_system_bucket)
            .unwrap()
            .add_expected_parallel_batch_worker_tick_bucket(data_cache_bucket)
            .unwrap()
            .add_expected_parallel_batch_worker_tick_bucket(scheduler_bucket)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_batch_worker_tick_buckets(),
        &[scheduler_bucket, data_cache_bucket, full_system_bucket],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_batch_worker_tick_buckets(),
        manifest.expected_parallel_batch_worker_tick_buckets(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                5,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                5,
                8,
                [partition(2), partition(3)],
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                10,
                14,
                [partition(4), partition(5), partition(6)],
                3,
            ),
            timeline_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                14,
                18,
                [partition(7), partition(8)],
                2,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_batch_worker_tick_buckets() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                8,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                9,
            ))
            .unwrap()
            .build()
            .unwrap();
    let wider_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-bucket"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(
                WorkloadParallelRemoteFlowScope::Scheduler,
                3,
                8,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger_scheduler.identity());
    assert_ne!(scheduler.identity(), wider_scheduler.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underfilled_parallel_batch_worker_tick_buckets() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(
            WorkloadParallelRemoteFlowScope::FullSystem,
            2,
            12,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchWorkerTickBucketSummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            worker_count: 2,
            minimum_ticks: 12,
        },
    );

    let underfilled_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            5,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            5,
            9,
            [partition(2), partition(3), partition(4)],
            3,
        )]);
    let underfilled = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underfilled_summary);
    assert_eq!(
        plan.verify_result(&underfilled).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerTickBucketBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            worker_count: 2,
            minimum_ticks: 12,
            actual_ticks: 5,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_batch_worker_tick_buckets() {
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerTickBucket::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            1,
            8,
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchWorkerTickBucket {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            worker_count: 1,
        },
    );
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerTickBucket::new(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
            0,
        )
        .unwrap_err(),
        WorkloadError::ZeroExpectedParallelBatchWorkerTickBucket {
            scope: WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            worker_count: 2,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            8,
        ))
        .unwrap()
        .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            9,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelBatchWorkerTickBucket {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            worker_count: 2,
        },
    );
}

#[test]
fn workload_manifest_records_parallel_batch_worker_tick_activity_expectations() {
    let scheduler_activity =
        expected_tick_activity(WorkloadParallelRemoteFlowScope::Scheduler, 2, 8);
    let data_cache_activity =
        expected_tick_activity(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3, 4);
    let full_system_activity =
        expected_tick_activity(WorkloadParallelRemoteFlowScope::FullSystem, 2, 12);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-worker-tick-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_activity(full_system_activity)
            .unwrap()
            .add_expected_parallel_batch_worker_tick_activity(data_cache_activity)
            .unwrap()
            .add_expected_parallel_batch_worker_tick_activity(scheduler_activity)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_batch_worker_tick_activity(),
        &[
            scheduler_activity,
            data_cache_activity,
            full_system_activity
        ],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_batch_worker_tick_activity(),
        manifest.expected_parallel_batch_worker_tick_activity(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                5,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                5,
                8,
                [partition(2), partition(3)],
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                10,
                14,
                [partition(4), partition(5), partition(6)],
                3,
            ),
            timeline_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                14,
                18,
                [partition(7), partition(8)],
                2,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_batch_worker_tick_activity() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_activity(expected_tick_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                8,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_activity(expected_tick_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                9,
            ))
            .unwrap()
            .build()
            .unwrap();
    let wider_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_activity(expected_tick_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                3,
                8,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger_scheduler.identity());
    assert_ne!(scheduler.identity(), wider_scheduler.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underfilled_parallel_batch_worker_tick_activity() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_activity(expected_tick_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            2,
            12,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchWorkerTickActivitySummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_worker_count: 2,
            minimum_ticks: 12,
        },
    );

    let underfilled_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            5,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            5,
            9,
            [partition(2), partition(3), partition(4)],
            3,
        )]);
    let underfilled = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underfilled_summary);
    assert_eq!(
        plan.verify_result(&underfilled).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerTickActivityBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_worker_count: 2,
            minimum_ticks: 12,
            actual_ticks: 9,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_batch_worker_tick_activity() {
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerTickActivity::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            1,
            8,
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchWorkerTickActivity {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_worker_count: 1,
        },
    );
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerTickActivity::new(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
            0,
        )
        .unwrap_err(),
        WorkloadError::ZeroExpectedParallelBatchWorkerTickActivity {
            scope: WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            minimum_worker_count: 2,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_batch_worker_tick_activity(expected_tick_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            8,
        ))
        .unwrap()
        .add_expected_parallel_batch_worker_tick_activity(expected_tick_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            9,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelBatchWorkerTickActivity {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_worker_count: 2,
        },
    );
}

#[test]
fn workload_manifest_records_parallel_batch_worker_tick_streak_expectations() {
    let scheduler_streak = expected_tick_streak(WorkloadParallelRemoteFlowScope::Scheduler, 2, 8);
    let data_cache_streak =
        expected_tick_streak(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3, 4);
    let full_system_streak =
        expected_tick_streak(WorkloadParallelRemoteFlowScope::FullSystem, 2, 8);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-worker-tick-streak"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_streak(full_system_streak)
            .unwrap()
            .add_expected_parallel_batch_worker_tick_streak(data_cache_streak)
            .unwrap()
            .add_expected_parallel_batch_worker_tick_streak(scheduler_streak)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_batch_worker_tick_streaks(),
        &[scheduler_streak, data_cache_streak, full_system_streak],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_batch_worker_tick_streaks(),
        manifest.expected_parallel_batch_worker_tick_streaks(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                5,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                5,
                8,
                [partition(2), partition(3)],
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                10,
                14,
                [partition(4), partition(5), partition(6)],
                3,
            ),
            timeline_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                14,
                18,
                [partition(7), partition(8)],
                2,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_batch_worker_tick_streaks() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-streak"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-streak"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                8,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-streak"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                9,
            ))
            .unwrap()
            .build()
            .unwrap();
    let wider_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-tick-streak"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(
                WorkloadParallelRemoteFlowScope::Scheduler,
                3,
                8,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger_scheduler.identity());
    assert_ne!(scheduler.identity(), wider_scheduler.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underfilled_parallel_batch_worker_tick_streaks() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(
            WorkloadParallelRemoteFlowScope::FullSystem,
            2,
            8,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchWorkerTickStreakSummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_worker_count: 2,
            minimum_consecutive_ticks: 8,
        },
    );

    let underfilled_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            5,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            6,
            10,
            [partition(2), partition(3), partition(4)],
            3,
        )]);
    let underfilled = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underfilled_summary);
    assert_eq!(
        plan.verify_result(&underfilled).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerTickStreakBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_worker_count: 2,
            minimum_consecutive_ticks: 8,
            actual_consecutive_ticks: 5,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_batch_worker_tick_streaks() {
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerTickStreak::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            1,
            8,
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchWorkerTickStreak {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_worker_count: 1,
        },
    );
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerTickStreak::new(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
            0,
        )
        .unwrap_err(),
        WorkloadError::ZeroExpectedParallelBatchWorkerTickStreak {
            scope: WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            minimum_worker_count: 2,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            8,
        ))
        .unwrap()
        .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            9,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelBatchWorkerTickStreak {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_worker_count: 2,
        },
    );
}

#[test]
fn workload_manifest_records_parallel_batch_worker_tick_expectations() {
    let scheduler_ticks = expected_worker_ticks(WorkloadParallelRemoteFlowScope::Scheduler, 16);
    let data_cache_ticks =
        expected_worker_ticks(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 12);
    let full_system_ticks = expected_worker_ticks(WorkloadParallelRemoteFlowScope::FullSystem, 28);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-worker-ticks"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_ticks(full_system_ticks)
            .unwrap()
            .add_expected_parallel_batch_worker_ticks(data_cache_ticks)
            .unwrap()
            .add_expected_parallel_batch_worker_ticks(scheduler_ticks)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_batch_worker_ticks(),
        &[scheduler_ticks, data_cache_ticks, full_system_ticks],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_batch_worker_ticks(),
        manifest.expected_parallel_batch_worker_ticks(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                5,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                5,
                8,
                [partition(2), partition(3)],
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                10,
                14,
                [partition(4), partition(5), partition(6)],
                3,
            ),
            timeline_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                14,
                18,
                [partition(7), partition(8)],
                2,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_batch_worker_ticks() {
    let base = rem6_workload::WorkloadManifest::builder(id("identity-worker-ticks"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-ticks"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_ticks(expected_worker_ticks(
                WorkloadParallelRemoteFlowScope::Scheduler,
                16,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-ticks"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_ticks(expected_worker_ticks(
                WorkloadParallelRemoteFlowScope::Scheduler,
                17,
            ))
            .unwrap()
            .build()
            .unwrap();
    let data_cache =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-ticks"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_worker_ticks(expected_worker_ticks(
                WorkloadParallelRemoteFlowScope::DataCacheScheduler,
                16,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger_scheduler.identity());
    assert_ne!(scheduler.identity(), data_cache.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underfilled_parallel_batch_worker_ticks() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_ticks(expected_worker_ticks(
            WorkloadParallelRemoteFlowScope::FullSystem,
            24,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchWorkerTicksSummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_worker_ticks: 24,
        },
    );

    let underfilled_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            5,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            6,
            10,
            [partition(2), partition(3), partition(4)],
            3,
        )]);
    let underfilled = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underfilled_summary);
    assert_eq!(
        plan.verify_result(&underfilled).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerTicksBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_worker_ticks: 24,
            actual_worker_ticks: 22,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_batch_worker_ticks() {
    assert_eq!(
        WorkloadExpectedParallelBatchWorkerTicks::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
        )
        .unwrap_err(),
        WorkloadError::ZeroExpectedParallelBatchWorkerTicks {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_batch_worker_ticks(expected_worker_ticks(
            WorkloadParallelRemoteFlowScope::Scheduler,
            16,
        ))
        .unwrap()
        .add_expected_parallel_batch_worker_ticks(expected_worker_ticks(
            WorkloadParallelRemoteFlowScope::Scheduler,
            17,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelBatchWorkerTicks {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );
}

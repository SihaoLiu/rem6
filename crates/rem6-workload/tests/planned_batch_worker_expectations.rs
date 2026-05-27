use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchWorkerBucket,
    WorkloadExpectedParallelBatchWorkerTickActivity, WorkloadExpectedParallelBatchWorkerTickBucket,
    WorkloadExpectedParallelBatchWorkerTickStreak, WorkloadExpectedParallelBatchWorkerTicks,
    WorkloadExpectedParallelWorkerActivity, WorkloadExpectedParallelWorkerUse, WorkloadId,
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerScope, WorkloadParallelExecutionSummary, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn partition(index: u32) -> PartitionId {
    PartitionId::new(index)
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
        rem6_workload::WorkloadManifest::builder(id("planned-batch-workers"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
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

#[test]
fn workload_replay_plan_checks_planned_parallel_batch_worker_expectations_directly() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_bucket(
            WorkloadExpectedParallelBatchWorkerBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedScheduler,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_bucket(
            WorkloadExpectedParallelBatchWorkerBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_tick_bucket(
            WorkloadExpectedParallelBatchWorkerTickBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedScheduler,
                2,
                4,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_tick_activity(
            WorkloadExpectedParallelBatchWorkerTickActivity::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                2,
                8,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_tick_streak(
            WorkloadExpectedParallelBatchWorkerTickStreak::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                2,
                8,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_ticks(
            WorkloadExpectedParallelBatchWorkerTicks::new_at_or_above(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                2,
                16,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let actual_only = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )]);
    let missing_planned = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_only);

    assert_eq!(
        plan.verify_result(&missing_planned).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerBucketBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::PlannedScheduler,
            worker_count: 2,
            minimum_batch_count: 1,
            actual_batch_count: 0,
        },
    );
}

#[test]
fn workload_replay_plan_checks_planned_parallel_worker_use_and_activity() {
    let plan = replay_plan()
        .add_expected_parallel_worker_use(
            WorkloadExpectedParallelWorkerUse::new(
                WorkloadParallelBatchWorkerScope::PlannedScheduler,
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_worker_activity(
            WorkloadExpectedParallelWorkerActivity::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                4,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0)],
            1,
        )])
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let actual_only = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )]);
    let missing_planned = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_only);

    assert_eq!(
        plan.verify_result(&missing_planned).unwrap_err(),
        WorkloadError::ExpectedParallelWorkerCountBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::PlannedScheduler,
            minimum_max_workers: 2,
            actual_max_workers: 0,
        },
    );
}

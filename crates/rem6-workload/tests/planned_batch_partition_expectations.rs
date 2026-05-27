use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchPartitionSet,
    WorkloadExpectedParallelBatchPartitionStreak, WorkloadExpectedParallelPartitionActivity,
    WorkloadExpectedParallelPartitionUse, WorkloadId, WorkloadParallelBatchPartitionScope,
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelExecutionSummary, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResult,
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
        rem6_workload::WorkloadManifest::builder(id("planned-batch-partitions"), boot_image())
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
fn workload_replay_plan_checks_planned_parallel_batch_partition_sets_directly() {
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_set(
            WorkloadExpectedParallelBatchPartitionSet::new(
                WorkloadParallelBatchPartitionScope::PlannedScheduler,
                [partition(0), partition(1)],
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_partition_set(
            WorkloadExpectedParallelBatchPartitionSet::new(
                WorkloadParallelBatchPartitionScope::PlannedDataCacheScheduler,
                [partition(2), partition(3)],
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_partition_set(
            WorkloadExpectedParallelBatchPartitionSet::new(
                WorkloadParallelBatchPartitionScope::PlannedFullSystem,
                [partition(0), partition(1)],
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(4), partition(5)],
            2,
        )])
        .with_parallel_scheduler_planned_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                4,
                8,
                [partition(1), partition(0)],
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            8,
            12,
            [partition(2), partition(3)],
            2,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let actual_only = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                4,
                8,
                [partition(1), partition(0)],
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            8,
            12,
            [partition(2), partition(3)],
            2,
        )]);
    let missing_planned = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_only);

    assert_eq!(
        plan.verify_result(&missing_planned).unwrap_err(),
        WorkloadError::ExpectedParallelBatchPartitionSetBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::PlannedScheduler,
            partitions: vec![0, 1],
            minimum_batch_count: 2,
            actual_batch_count: 0,
        },
    );
}

#[test]
fn workload_replay_plan_checks_planned_parallel_partition_streak_use_and_activity() {
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_streak(
            WorkloadExpectedParallelBatchPartitionStreak::new(
                WorkloadParallelBatchPartitionScope::PlannedFullSystem,
                [partition(0), partition(1)],
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_partition_use(
            WorkloadExpectedParallelPartitionUse::new(
                WorkloadParallelBatchPartitionScope::PlannedFullSystem,
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_partition_activity(
            WorkloadExpectedParallelPartitionActivity::new(
                WorkloadParallelBatchPartitionScope::PlannedScheduler,
                partition(0),
                2,
                0,
                0,
                0,
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
        .with_parallel_scheduler_planned_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                4,
                8,
                [partition(1), partition(0)],
                2,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let actual_only = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                4,
                8,
                [partition(1), partition(0)],
                2,
            ),
        ]);
    let missing_planned = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_only);

    assert_eq!(
        plan.verify_result(&missing_planned).unwrap_err(),
        WorkloadError::ExpectedParallelBatchPartitionStreakBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::PlannedFullSystem,
            partitions: vec![0, 1],
            minimum_consecutive_batch_count: 2,
            actual_consecutive_batch_count: 0,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_actual_partition_evidence_for_planned_scopes() {
    let use_plan = replay_plan()
        .add_expected_parallel_partition_use(
            WorkloadExpectedParallelPartitionUse::new(
                WorkloadParallelBatchPartitionScope::PlannedFullSystem,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let activity_plan = replay_plan()
        .add_expected_parallel_partition_activity(
            WorkloadExpectedParallelPartitionActivity::new(
                WorkloadParallelBatchPartitionScope::PlannedScheduler,
                partition(0),
                2,
                0,
                0,
                0,
            )
            .unwrap(),
        )
        .unwrap();
    let actual_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                4,
                8,
                [partition(1), partition(0)],
                2,
            ),
        ]);

    let use_result = WorkloadResult::new(use_plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_summary.clone());
    assert_eq!(
        use_plan.verify_result(&use_result).unwrap_err(),
        WorkloadError::ExpectedParallelPartitionCountBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::PlannedFullSystem,
            minimum_active_partitions: 2,
            actual_active_partitions: 0,
        },
    );

    let activity_result = WorkloadResult::new(activity_plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_summary);
    assert_eq!(
        activity_plan.verify_result(&activity_result).unwrap_err(),
        WorkloadError::ExpectedParallelPartitionActivityBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::PlannedScheduler,
            partition: 0,
            minimum_worker_count: 2,
            actual_worker_count: 0,
            minimum_dispatch_count: 0,
            actual_dispatch_count: 0,
            minimum_remote_send_count: 0,
            actual_remote_send_count: 0,
            minimum_remote_receive_count: 0,
            actual_remote_receive_count: 0,
        },
    );
}

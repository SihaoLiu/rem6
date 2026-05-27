use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchWorkerTickActivity,
    WorkloadExpectedParallelBatchWorkerTickBucket, WorkloadExpectedParallelBatchWorkerTickStreak,
    WorkloadExpectedParallelBatchWorkerTicks, WorkloadId, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord, WorkloadParallelBatchWorkerScope,
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
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-batch-worker-tick-merge"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_tick_bucket(
    worker_count: usize,
    minimum_ticks: u64,
) -> WorkloadExpectedParallelBatchWorkerTickBucket {
    WorkloadExpectedParallelBatchWorkerTickBucket::new(
        WorkloadParallelRemoteFlowScope::FullSystem,
        worker_count,
        minimum_ticks,
    )
    .unwrap()
}

fn expected_tick_activity(
    minimum_worker_count: usize,
    minimum_ticks: u64,
) -> WorkloadExpectedParallelBatchWorkerTickActivity {
    WorkloadExpectedParallelBatchWorkerTickActivity::new(
        WorkloadParallelRemoteFlowScope::FullSystem,
        minimum_worker_count,
        minimum_ticks,
    )
    .unwrap()
}

fn expected_tick_streak(
    minimum_worker_count: usize,
    minimum_consecutive_ticks: u64,
) -> WorkloadExpectedParallelBatchWorkerTickStreak {
    WorkloadExpectedParallelBatchWorkerTickStreak::new(
        WorkloadParallelRemoteFlowScope::FullSystem,
        minimum_worker_count,
        minimum_consecutive_ticks,
    )
    .unwrap()
}

fn expected_worker_ticks(
    minimum_worker_count: usize,
    minimum_worker_ticks: u64,
) -> WorkloadExpectedParallelBatchWorkerTicks {
    WorkloadExpectedParallelBatchWorkerTicks::new_at_or_above(
        WorkloadParallelRemoteFlowScope::FullSystem,
        minimum_worker_count,
        minimum_worker_ticks,
    )
    .unwrap()
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

fn cpu_cache_timeline_summary() -> WorkloadParallelExecutionSummary {
    WorkloadParallelExecutionSummary::default()
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
        )])
}

#[test]
fn workload_replay_plan_rejects_weak_explicit_full_system_batch_worker_tick_bucket() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(2, 5))
        .unwrap();
    let summary = cpu_cache_timeline_summary()
        .with_full_system_parallel_scheduler_batch_worker_count_tick_summaries([(2, 2)]);

    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_for_worker_count(2),
        5,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerTickBucketBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            worker_count: 2,
            minimum_ticks: 5,
            actual_ticks: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_weak_explicit_full_system_batch_worker_tick_activity() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_activity(expected_tick_activity(2, 9))
        .unwrap();
    let summary = cpu_cache_timeline_summary()
        .with_full_system_parallel_scheduler_batch_worker_count_tick_summaries([(2, 2)]);

    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_at_or_above(2),
        9,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerTickActivityBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            minimum_worker_count: 2,
            minimum_ticks: 9,
            actual_ticks: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_weak_explicit_full_system_batch_worker_tick_streak() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(2, 8))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            8,
            [partition(0), partition(1)],
            2,
        )])
        .with_full_system_parallel_scheduler_batch_worker_tick_streak_summaries([(2, 3)]);

    assert_eq!(
        summary.full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(2),
        8,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerTickStreakBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            minimum_worker_count: 2,
            minimum_consecutive_ticks: 8,
            actual_consecutive_ticks: 3,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_weak_explicit_full_system_batch_worker_ticks() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_ticks(expected_worker_ticks(2, 22))
        .unwrap();
    let summary = cpu_cache_timeline_summary()
        .with_full_system_parallel_scheduler_batch_worker_count_tick_summaries([(2, 2)]);

    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_ticks_at_or_above(2),
        22,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerTicksBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            minimum_worker_count: 2,
            minimum_worker_ticks: 22,
            actual_worker_ticks: 4,
        },
    );
}

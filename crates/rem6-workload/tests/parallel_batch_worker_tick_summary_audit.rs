use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchWorkerTickBucket,
    WorkloadExpectedParallelBatchWorkerTickStreak, WorkloadId, WorkloadParallelBatchWorkerScope,
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
        id("parallel-batch-worker-tick-summary-audit"),
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

#[test]
fn workload_replay_plan_rejects_duplicate_explicit_full_system_batch_worker_tick_summaries() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(2, 4))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_worker_count_tick_summaries([(2, 2), (2, 2)]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchWorkerTickSummary {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            worker_count: 2,
            ticks: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_empty_explicit_full_system_batch_worker_tick_summaries() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_bucket(expected_tick_bucket(2, 1))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_worker_count_tick_summaries([(0, 1)]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchWorkerTickSummary {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            worker_count: 0,
            ticks: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_explicit_full_system_batch_worker_tick_streak_summaries()
{
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(2, 4))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_worker_tick_streak_summaries([(2, 2), (2, 2)]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchWorkerTickSummary {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            worker_count: 2,
            ticks: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_empty_explicit_full_system_batch_worker_tick_streak_summaries() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_tick_streak(expected_tick_streak(2, 1))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_worker_tick_streak_summaries([(0, 1)]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchWorkerTickSummary {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            worker_count: 0,
            ticks: 1,
        },
    );
}

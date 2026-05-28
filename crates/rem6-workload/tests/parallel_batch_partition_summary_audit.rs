use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchPartitionSet,
    WorkloadExpectedParallelBatchPartitionStreak, WorkloadId, WorkloadParallelBatchPartitionScope,
    WorkloadParallelBatchPartitionSet, WorkloadParallelBatchPartitionStreak,
    WorkloadParallelExecutionSummary, WorkloadParallelRemoteFlowScope, WorkloadReplayPlan,
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
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-batch-partition-summary-audit"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_partition_set(
    partitions: impl IntoIterator<Item = PartitionId>,
    minimum_batch_count: usize,
) -> WorkloadExpectedParallelBatchPartitionSet {
    WorkloadExpectedParallelBatchPartitionSet::new(
        WorkloadParallelRemoteFlowScope::FullSystem,
        partitions,
        minimum_batch_count,
    )
    .unwrap()
}

fn expected_partition_streak(
    partitions: impl IntoIterator<Item = PartitionId>,
    minimum_consecutive_batch_count: usize,
) -> WorkloadExpectedParallelBatchPartitionStreak {
    WorkloadExpectedParallelBatchPartitionStreak::new(
        WorkloadParallelRemoteFlowScope::FullSystem,
        partitions,
        minimum_consecutive_batch_count,
    )
    .unwrap()
}

#[test]
fn workload_replay_plan_rejects_duplicate_explicit_full_system_batch_partition_sets() {
    let cpu = partition(0);
    let cache = partition(2);
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_set(expected_partition_set([cpu, cache], 3))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([cpu, cache], 1),
            WorkloadParallelBatchPartitionSet::new([cache, cpu], 2),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchPartitionSummary {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partitions: vec![0, 2],
            count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_empty_explicit_full_system_batch_partition_sets() {
    let cpu = partition(0);
    let cache = partition(2);
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_set(expected_partition_set([cpu, cache], 1))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([], 1),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchPartitionSummary {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partitions: vec![],
            count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_explicit_full_system_batch_partition_streaks() {
    let cpu = partition(0);
    let cache = partition(2);
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_streak(expected_partition_streak([cpu, cache], 3))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_partition_streaks([
            WorkloadParallelBatchPartitionStreak::new([cpu, cache], 1),
            WorkloadParallelBatchPartitionStreak::new([cache, cpu], 2),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchPartitionSummary {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partitions: vec![0, 2],
            count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_bad_full_system_partition_streaks_used_as_partition_sets() {
    let cpu = partition(0);
    let cache = partition(2);
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_set(expected_partition_set([cpu, cache], 3))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_partition_streaks([
            WorkloadParallelBatchPartitionStreak::new([cpu, cache], 1),
            WorkloadParallelBatchPartitionStreak::new([cache, cpu], 2),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchPartitionSummary {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partitions: vec![0, 2],
            count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_empty_explicit_full_system_batch_partition_streaks() {
    let cpu = partition(0);
    let cache = partition(2);
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_streak(expected_partition_streak([cpu, cache], 1))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_full_system_parallel_scheduler_batch_partition_streaks([
            WorkloadParallelBatchPartitionStreak::new([], 1),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchPartitionSummary {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partitions: vec![],
            count: 1,
        },
    );
}

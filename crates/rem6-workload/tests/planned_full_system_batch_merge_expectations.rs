use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchTimelineRecord,
    WorkloadExpectedParallelBatchWorkerBucket, WorkloadId, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord, WorkloadParallelBatchTimelineScope,
    WorkloadParallelBatchWorkerScope, WorkloadParallelExecutionSummary, WorkloadReplayPlan,
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
        id("planned-full-system-batch-merge"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn partition(index: u32) -> PartitionId {
    PartitionId::new(index)
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

fn expected_timeline(
    scope: WorkloadParallelBatchTimelineScope,
    batch_scope: WorkloadParallelBatchScope,
    start_tick: u64,
    horizon: u64,
    partitions: impl IntoIterator<Item = PartitionId>,
    worker_count: usize,
) -> WorkloadExpectedParallelBatchTimelineRecord {
    WorkloadExpectedParallelBatchTimelineRecord::new(
        scope,
        batch_scope,
        start_tick,
        horizon,
        partitions,
        worker_count,
    )
    .unwrap()
}

#[test]
fn workload_replay_plan_accepts_covered_explicit_planned_full_system_batch_timeline() {
    let cpu = timeline_record(
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0), partition(1)],
        2,
    );
    let gpu = timeline_record(
        WorkloadParallelBatchScope::GpuDmaScheduler,
        4,
        8,
        [partition(2), partition(3)],
        2,
    );
    let plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(expected_timeline(
            WorkloadParallelBatchTimelineScope::PlannedFullSystem,
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        ))
        .unwrap()
        .add_expected_parallel_batch_timeline_record(expected_timeline(
            WorkloadParallelBatchTimelineScope::PlannedFullSystem,
            WorkloadParallelBatchScope::GpuDmaScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([cpu.clone()])
        .with_gpu_dma_scheduler_planned_batch_timeline([gpu.clone()])
        .with_full_system_parallel_scheduler_planned_batch_timeline([cpu, gpu]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_weaker_explicit_planned_full_system_batch_timeline() {
    let cpu = timeline_record(
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0), partition(1)],
        2,
    );
    let gpu = timeline_record(
        WorkloadParallelBatchScope::GpuDmaScheduler,
        4,
        8,
        [partition(2), partition(3)],
        2,
    );
    let plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(expected_timeline(
            WorkloadParallelBatchTimelineScope::PlannedFullSystem,
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([cpu.clone()])
        .with_gpu_dma_scheduler_planned_batch_timeline([gpu])
        .with_full_system_parallel_scheduler_planned_batch_timeline([cpu]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelBatchTimelineMergeSummary {
            scope: WorkloadParallelBatchTimelineScope::PlannedFullSystem,
            batch_scope: WorkloadParallelBatchScope::GpuDmaScheduler,
            start_tick: 4,
            horizon: 8,
            partitions: vec![2, 3],
            worker_count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_weaker_planned_full_system_worker_bucket_merge() {
    let cpu = timeline_record(
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0), partition(1)],
        2,
    );
    let gpu = timeline_record(
        WorkloadParallelBatchScope::GpuDmaScheduler,
        4,
        8,
        [partition(2), partition(3)],
        2,
    );
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_bucket(
            WorkloadExpectedParallelBatchWorkerBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([cpu.clone()])
        .with_gpu_dma_scheduler_planned_batch_timeline([gpu])
        .with_full_system_parallel_scheduler_planned_batch_timeline([cpu]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelBatchTimelineMergeSummary {
            scope: WorkloadParallelBatchTimelineScope::PlannedFullSystem,
            batch_scope: WorkloadParallelBatchScope::GpuDmaScheduler,
            start_tick: 4,
            horizon: 8,
            partitions: vec![2, 3],
            worker_count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_planned_full_system_batch_records() {
    let cpu = timeline_record(
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0), partition(1)],
        2,
    );
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_bucket(
            WorkloadExpectedParallelBatchWorkerBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([cpu.clone()])
        .with_full_system_parallel_scheduler_planned_batch_timeline([cpu.clone(), cpu]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::PlannedFullSystem,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0, 1],
            worker_count: 2,
        },
    );
}

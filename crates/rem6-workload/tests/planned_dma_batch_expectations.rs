use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchPartitionSet,
    WorkloadExpectedParallelBatchTimelineRecord, WorkloadExpectedParallelBatchWorkerBucket,
    WorkloadId, WorkloadParallelBatchPartitionScope, WorkloadParallelBatchScope,
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
        rem6_workload::WorkloadManifest::builder(id("planned-dma-batches"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn timeline_record(
    batch_scope: WorkloadParallelBatchScope,
    start_tick: u64,
    horizon: u64,
    partitions: impl IntoIterator<Item = PartitionId>,
    worker_count: usize,
) -> WorkloadParallelBatchTimelineRecord {
    WorkloadParallelBatchTimelineRecord::new(
        batch_scope,
        start_tick,
        horizon,
        partitions,
        worker_count,
    )
}

#[test]
fn workload_summary_preserves_planned_dma_batch_timelines() {
    let gpu = timeline_record(
        WorkloadParallelBatchScope::GpuDmaScheduler,
        0,
        4,
        [partition(10), partition(11)],
        2,
    );
    let accelerator = timeline_record(
        WorkloadParallelBatchScope::AcceleratorDmaScheduler,
        4,
        8,
        [partition(20), partition(21)],
        2,
    );
    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_planned_batch_timeline([gpu.clone()])
        .with_accelerator_dma_scheduler_planned_batch_timeline([accelerator.clone()]);

    assert_eq!(
        summary.gpu_dma_scheduler_planned_batch_timeline(),
        std::slice::from_ref(&gpu),
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_planned_batch_timeline(),
        std::slice::from_ref(&accelerator),
    );
    assert_eq!(
        summary.dma_scheduler_planned_batch_timeline(),
        vec![gpu.clone(), accelerator.clone()],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_timeline(),
        vec![gpu, accelerator],
    );
}

#[test]
fn workload_replay_plan_checks_planned_dma_timeline_worker_and_partition_contracts() {
    let gpu = timeline_record(
        WorkloadParallelBatchScope::GpuDmaScheduler,
        0,
        4,
        [partition(10), partition(11)],
        2,
    );
    let accelerator = timeline_record(
        WorkloadParallelBatchScope::AcceleratorDmaScheduler,
        4,
        8,
        [partition(20), partition(21)],
        2,
    );
    let plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(
            WorkloadExpectedParallelBatchTimelineRecord::new(
                WorkloadParallelBatchTimelineScope::PlannedGpuDmaScheduler,
                WorkloadParallelBatchScope::GpuDmaScheduler,
                0,
                4,
                [partition(10), partition(11)],
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_timeline_record(
            WorkloadExpectedParallelBatchTimelineRecord::new(
                WorkloadParallelBatchTimelineScope::PlannedAcceleratorDmaScheduler,
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                4,
                8,
                [partition(20), partition(21)],
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_timeline_record(
            WorkloadExpectedParallelBatchTimelineRecord::new(
                WorkloadParallelBatchTimelineScope::PlannedDmaScheduler,
                WorkloadParallelBatchScope::GpuDmaScheduler,
                0,
                4,
                [partition(10), partition(11)],
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_timeline_record(
            WorkloadExpectedParallelBatchTimelineRecord::new(
                WorkloadParallelBatchTimelineScope::PlannedDmaScheduler,
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                4,
                8,
                [partition(20), partition(21)],
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_timeline_record(
            WorkloadExpectedParallelBatchTimelineRecord::new(
                WorkloadParallelBatchTimelineScope::PlannedFullSystem,
                WorkloadParallelBatchScope::GpuDmaScheduler,
                0,
                4,
                [partition(10), partition(11)],
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_timeline_record(
            WorkloadExpectedParallelBatchTimelineRecord::new(
                WorkloadParallelBatchTimelineScope::PlannedFullSystem,
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                4,
                8,
                [partition(20), partition(21)],
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_bucket(
            WorkloadExpectedParallelBatchWorkerBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedDmaScheduler,
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_partition_set(
            WorkloadExpectedParallelBatchPartitionSet::new(
                WorkloadParallelBatchPartitionScope::PlannedAcceleratorDmaScheduler,
                [partition(20), partition(21)],
                1,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_batch_timeline([gpu.clone()])
        .with_gpu_dma_scheduler_planned_batch_timeline([gpu])
        .with_accelerator_dma_scheduler_planned_batch_timeline([accelerator]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let actual_only = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            0,
            4,
            [partition(10), partition(11)],
            2,
        )])
        .with_accelerator_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::AcceleratorDmaScheduler,
            4,
            8,
            [partition(20), partition(21)],
            2,
        )]);
    let missing_planned = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_only);

    assert_eq!(
        plan.verify_result(&missing_planned).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerBucketBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::PlannedDmaScheduler,
            worker_count: 2,
            minimum_batch_count: 2,
            actual_batch_count: 0,
        },
    );
}

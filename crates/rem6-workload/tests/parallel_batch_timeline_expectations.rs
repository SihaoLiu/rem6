use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchTimelineRecord, WorkloadId,
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchTimelineScope, WorkloadParallelExecutionSummary, WorkloadReplayPlan,
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
    let manifest = rem6_workload::WorkloadManifest::builder(id("batch-timeline"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
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
fn workload_manifest_records_parallel_batch_timeline_expectations() {
    let scheduler = expected_timeline(
        WorkloadParallelBatchTimelineScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(1), partition(0)],
        2,
    );
    let data_cache = expected_timeline(
        WorkloadParallelBatchTimelineScope::DataCacheScheduler,
        WorkloadParallelBatchScope::DataCacheScheduler,
        4,
        8,
        [partition(2), partition(3)],
        2,
    );
    let full_system_scheduler = expected_timeline(
        WorkloadParallelBatchTimelineScope::FullSystem,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0), partition(1)],
        2,
    );
    let full_system_data_cache = expected_timeline(
        WorkloadParallelBatchTimelineScope::FullSystem,
        WorkloadParallelBatchScope::DataCacheScheduler,
        4,
        8,
        [partition(2), partition(3)],
        2,
    );
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-batch-timeline"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_timeline_record(full_system_data_cache.clone())
            .unwrap()
            .add_expected_parallel_batch_timeline_record(full_system_scheduler.clone())
            .unwrap()
            .add_expected_parallel_batch_timeline_record(data_cache.clone())
            .unwrap()
            .add_expected_parallel_batch_timeline_record(scheduler.clone())
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_batch_timeline_records(),
        &[
            scheduler.clone(),
            data_cache.clone(),
            full_system_scheduler.clone(),
            full_system_data_cache.clone()
        ],
    );
    assert_eq!(
        scheduler.scope(),
        WorkloadParallelBatchTimelineScope::Scheduler
    );
    assert_eq!(
        scheduler.batch_scope(),
        WorkloadParallelBatchScope::Scheduler
    );
    assert_eq!(scheduler.partitions(), &[partition(0), partition(1)]);
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_batch_timeline_records(),
        manifest.expected_parallel_batch_timeline_records(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_checks_dma_scheduler_parallel_batch_timelines_directly() {
    let gpu_dma = expected_timeline(
        WorkloadParallelBatchTimelineScope::GpuDmaScheduler,
        WorkloadParallelBatchScope::GpuDmaScheduler,
        8,
        12,
        [partition(3), partition(4)],
        2,
    );
    let accelerator_dma = expected_timeline(
        WorkloadParallelBatchTimelineScope::AcceleratorDmaScheduler,
        WorkloadParallelBatchScope::AcceleratorDmaScheduler,
        12,
        18,
        [partition(5), partition(6)],
        2,
    );
    let plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(gpu_dma.clone())
        .unwrap()
        .add_expected_parallel_batch_timeline_record(accelerator_dma)
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            8,
            12,
            [partition(3), partition(4)],
            2,
        )])
        .with_accelerator_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::AcceleratorDmaScheduler,
            12,
            18,
            [partition(5), partition(6)],
            2,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let exact_gpu_plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(gpu_dma)
        .unwrap();
    let extra_gpu_summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::GpuDmaScheduler,
                8,
                12,
                [partition(3), partition(4)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::GpuDmaScheduler,
                16,
                20,
                [partition(4)],
                1,
            ),
        ]);
    let extra_gpu = WorkloadResult::new(exact_gpu_plan.manifest_identity(), 32)
        .with_parallel_execution_summary(extra_gpu_summary);

    assert_eq!(
        exact_gpu_plan.verify_result(&extra_gpu).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::GpuDmaScheduler,
            batch_scope: WorkloadParallelBatchScope::GpuDmaScheduler,
            start_tick: 16,
            horizon: 20,
            partitions: vec![4],
            worker_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_uses_explicit_full_system_batch_timeline_evidence() {
    let cpu = partition(0);
    let cache = partition(1);
    let dma = partition(2);
    let expected = expected_timeline(
        WorkloadParallelBatchTimelineScope::FullSystem,
        WorkloadParallelBatchScope::Scheduler,
        0,
        6,
        [cpu, cache, dma],
        3,
    );
    let plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(expected)
        .unwrap();
    let scoped_scheduler = timeline_record(
        WorkloadParallelBatchScope::Scheduler,
        10,
        14,
        [cpu, cache],
        2,
    );
    let global = timeline_record(
        WorkloadParallelBatchScope::Scheduler,
        0,
        6,
        [cpu, cache, dma],
        3,
    );
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([scoped_scheduler])
        .with_full_system_parallel_scheduler_batch_timeline([global.clone()]);
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_timeline(),
        vec![global],
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_batch_timeline_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-timeline"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let start_four =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-timeline"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_timeline_record(expected_timeline(
                WorkloadParallelBatchTimelineScope::Scheduler,
                WorkloadParallelBatchScope::Scheduler,
                4,
                8,
                [partition(0), partition(1)],
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let start_eight =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-timeline"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_timeline_record(expected_timeline(
                WorkloadParallelBatchTimelineScope::Scheduler,
                WorkloadParallelBatchScope::Scheduler,
                8,
                12,
                [partition(0), partition(1)],
                2,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), start_four.identity());
    assert_ne!(start_four.identity(), start_eight.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_unexpected_parallel_batch_timeline_records() {
    let plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(expected_timeline(
            WorkloadParallelBatchTimelineScope::Scheduler,
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchTimelineSummary {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0, 1],
            worker_count: 2,
        },
    );

    let wrong_timeline = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            4,
            8,
            [partition(0), partition(1)],
            2,
        )]);
    let wrong = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(wrong_timeline);
    assert_eq!(
        plan.verify_result(&wrong).unwrap_err(),
        WorkloadError::ExpectedParallelBatchTimelineRecordMissing {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0, 1],
            worker_count: 2,
        },
    );

    let extra_timeline = WorkloadParallelExecutionSummary::default()
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
                [partition(1)],
                1,
            ),
        ]);
    let extra = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(extra_timeline);
    assert_eq!(
        plan.verify_result(&extra).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 4,
            horizon: 8,
            partitions: vec![1],
            worker_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_actual_parallel_batch_timeline_records() {
    let plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(expected_timeline(
            WorkloadParallelBatchTimelineScope::Scheduler,
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        ))
        .unwrap();
    let duplicate_timeline = WorkloadParallelExecutionSummary::default()
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
                0,
                4,
                [partition(0), partition(1)],
                2,
            ),
        ]);
    let duplicate = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(duplicate_timeline);

    assert_eq!(
        plan.verify_result(&duplicate).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0, 1],
            worker_count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_malformed_actual_parallel_batch_timeline_records() {
    let plan = replay_plan()
        .add_expected_parallel_batch_timeline_record(expected_timeline(
            WorkloadParallelBatchTimelineScope::Scheduler,
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        ))
        .unwrap();
    let malformed_timeline = WorkloadParallelExecutionSummary::default()
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
                9,
                5,
                [partition(0), partition(1)],
                2,
            ),
        ]);
    let malformed = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(malformed_timeline);

    assert_eq!(
        plan.verify_result(&malformed).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 9,
            horizon: 5,
            partitions: vec![0, 1],
            worker_count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_batch_timeline_records() {
    let zero_worker = WorkloadExpectedParallelBatchTimelineRecord::new(
        WorkloadParallelBatchTimelineScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0)],
        0,
    );
    assert_eq!(
        zero_worker.unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0],
            worker_count: 0,
        },
    );

    let one_worker = WorkloadExpectedParallelBatchTimelineRecord::new(
        WorkloadParallelBatchTimelineScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0), partition(1)],
        1,
    );
    assert_eq!(
        one_worker.unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0, 1],
            worker_count: 1,
        },
    );

    let one_partition = WorkloadExpectedParallelBatchTimelineRecord::new(
        WorkloadParallelBatchTimelineScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0)],
        2,
    );
    assert_eq!(
        one_partition.unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0],
            worker_count: 2,
        },
    );

    let zero_duration = WorkloadExpectedParallelBatchTimelineRecord::new(
        WorkloadParallelBatchTimelineScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        4,
        4,
        [partition(0), partition(1)],
        2,
    );
    assert_eq!(
        zero_duration.unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 4,
            horizon: 4,
            partitions: vec![0, 1],
            worker_count: 2,
        },
    );

    let expected = expected_timeline(
        WorkloadParallelBatchTimelineScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0), partition(1)],
        2,
    );
    assert_eq!(
        replay_plan()
            .add_expected_parallel_batch_timeline_record(expected.clone())
            .unwrap()
            .add_expected_parallel_batch_timeline_record(expected)
            .unwrap_err(),
        WorkloadError::DuplicateExpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelBatchTimelineScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0, 1],
            worker_count: 2,
        },
    );
}

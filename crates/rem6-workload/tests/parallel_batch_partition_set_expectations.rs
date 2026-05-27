use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchPartitionSet, WorkloadId,
    WorkloadParallelBatchPartitionScope, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchTimelineScope, WorkloadParallelExecutionSummary,
    WorkloadParallelRemoteFlowScope, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
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
        rem6_workload::WorkloadManifest::builder(id("parallel-batch-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_set(
    scope: WorkloadParallelRemoteFlowScope,
    partitions: impl IntoIterator<Item = PartitionId>,
    minimum_batch_count: usize,
) -> WorkloadExpectedParallelBatchPartitionSet {
    WorkloadExpectedParallelBatchPartitionSet::new(scope, partitions, minimum_batch_count).unwrap()
}

fn expected_dma_set(
    scope: WorkloadParallelBatchPartitionScope,
    partitions: impl IntoIterator<Item = PartitionId>,
    minimum_batch_count: usize,
) -> WorkloadExpectedParallelBatchPartitionSet {
    WorkloadExpectedParallelBatchPartitionSet::new(scope, partitions, minimum_batch_count).unwrap()
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
fn workload_replay_plan_checks_dma_scheduler_batch_partition_sets_directly() {
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_set(expected_dma_set(
            WorkloadParallelBatchPartitionScope::GpuDmaScheduler,
            [partition(3), partition(4)],
            2,
        ))
        .unwrap()
        .add_expected_parallel_batch_partition_set(expected_dma_set(
            WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler,
            [partition(5), partition(6)],
            1,
        ))
        .unwrap()
        .add_expected_parallel_batch_partition_set(expected_dma_set(
            WorkloadParallelBatchPartitionScope::FullSystem,
            [partition(3), partition(4)],
            2,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::GpuDmaScheduler,
                8,
                10,
                [partition(3), partition(4)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::GpuDmaScheduler,
                10,
                12,
                [partition(4), partition(3)],
                2,
            ),
        ])
        .with_accelerator_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::AcceleratorDmaScheduler,
            12,
            15,
            [partition(5), partition(6)],
            2,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let failing_plan = replay_plan()
        .add_expected_parallel_batch_partition_set(expected_dma_set(
            WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler,
            [partition(5), partition(6)],
            2,
        ))
        .unwrap();
    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_accelerator_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::AcceleratorDmaScheduler,
            12,
            15,
            [partition(5), partition(6)],
            2,
        )]);
    let underactive = WorkloadResult::new(failing_plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);

    assert_eq!(
        failing_plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedParallelBatchPartitionSetBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler,
            partitions: vec![5, 6],
            minimum_batch_count: 2,
            actual_batch_count: 1,
        },
    );
}

#[test]
fn workload_manifest_records_parallel_batch_partition_set_expectations() {
    let scheduler_set = expected_set(
        WorkloadParallelRemoteFlowScope::Scheduler,
        [partition(1), partition(0)],
        2,
    );
    let data_cache_set = expected_set(
        WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        [partition(3), partition(2), partition(1)],
        1,
    );
    let full_system_set = expected_set(
        WorkloadParallelRemoteFlowScope::FullSystem,
        [partition(2), partition(0)],
        3,
    );
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-batch-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_partition_set(full_system_set.clone())
            .unwrap()
            .add_expected_parallel_batch_partition_set(data_cache_set.clone())
            .unwrap()
            .add_expected_parallel_batch_partition_set(scheduler_set.clone())
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_batch_partition_sets(),
        &[
            scheduler_set.clone(),
            data_cache_set.clone(),
            full_system_set.clone()
        ],
    );
    assert_eq!(scheduler_set.partitions(), &[partition(0), partition(1)]);
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_batch_partition_sets(),
        manifest.expected_parallel_batch_partition_sets(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(1)], 2),
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(2)], 1),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(2)], 2),
            WorkloadParallelBatchPartitionSet::new([partition(1), partition(2), partition(3)], 1),
        ]);
    assert_eq!(
        summary.parallel_scheduler_batch_count_for_partition_set([partition(0), partition(1)]),
        2,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_for_partition_set([
            partition(1),
            partition(2),
            partition(3),
        ]),
        1,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_partition_set([
            partition(0),
            partition(2),
        ]),
        3,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_uses_explicit_full_system_partition_set_evidence() {
    let cpu = partition(0);
    let cache = partition(2);
    let dma = partition(3);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-batch-partitions-full-system-sets"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::FullSystem,
            [cpu, cache],
            4,
        ))
        .unwrap()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::FullSystem,
            [cpu, cache, dma],
            2,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([WorkloadParallelBatchPartitionSet::new(
            [cpu, cache],
            1,
        )])
        .with_full_system_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([cpu, cache], 4),
            WorkloadParallelBatchPartitionSet::new([cpu, cache, dma], 2),
        ]);
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_partition_sets(),
        vec![
            WorkloadParallelBatchPartitionSet::new([cpu, cache], 4),
            WorkloadParallelBatchPartitionSet::new([cpu, cache, dma], 2),
        ],
    );

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_batch_partition_sets() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let two_partitions =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_partition_set(expected_set(
                WorkloadParallelRemoteFlowScope::Scheduler,
                [partition(0), partition(1)],
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_partition_set(expected_set(
                WorkloadParallelRemoteFlowScope::Scheduler,
                [partition(0), partition(1)],
                3,
            ))
            .unwrap()
            .build()
            .unwrap();
    let different_partitions =
        rem6_workload::WorkloadManifest::builder(id("identity-batch-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_batch_partition_set(expected_set(
                WorkloadParallelRemoteFlowScope::Scheduler,
                [partition(0), partition(2)],
                2,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), two_partitions.identity());
    assert_ne!(two_partitions.identity(), stronger.identity());
    assert_ne!(two_partitions.identity(), different_partitions.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_parallel_batch_partition_sets() {
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::FullSystem,
            [partition(0), partition(2)],
            3,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchPartitionSetSummary {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partitions: vec![0, 2],
            minimum_batch_count: 3,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([WorkloadParallelBatchPartitionSet::new(
            [partition(0), partition(2)],
            1,
        )])
        .with_data_cache_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(1)], 4),
        ]);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedParallelBatchPartitionSetBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partitions: vec![0, 2],
            minimum_batch_count: 3,
            actual_batch_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_malformed_timeline_for_batch_partition_sets() {
    let plan = replay_plan()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::Scheduler,
            [partition(0), partition(1)],
            1,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
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
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
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
fn workload_replay_plan_derives_batch_partition_sets_from_streaks() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-batch-partitions-from-streaks"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::Scheduler,
            [partition(0), partition(1)],
            3,
        ))
        .unwrap()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            [partition(10), partition(11), partition(12)],
            4,
        ))
        .unwrap()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::FullSystem,
            [partition(0), partition(2)],
            5,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_streak_sequence([
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(1)], 3),
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(2)], 2),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_streak_sequence([
            WorkloadParallelBatchPartitionSet::new(
                [partition(10), partition(11), partition(12)],
                4,
            ),
            WorkloadParallelBatchPartitionSet::new([partition(0), partition(2)], 3),
        ]);

    assert_eq!(
        summary.parallel_scheduler_batch_count_for_partition_set([partition(0), partition(1)]),
        3,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_for_partition_set([
            partition(10),
            partition(11),
            partition(12),
        ]),
        4,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_partition_set([
            partition(0),
            partition(2),
        ]),
        5,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_batch_partition_sets() {
    let single_partition = WorkloadExpectedParallelBatchPartitionSet::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        [partition(0)],
        2,
    )
    .unwrap_err();
    assert_eq!(
        single_partition,
        WorkloadError::InvalidExpectedParallelBatchPartitionSet {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            partitions: vec![0],
        },
    );

    let zero_batch_count = WorkloadExpectedParallelBatchPartitionSet::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        [partition(0), partition(1)],
        0,
    )
    .unwrap_err();
    assert_eq!(
        zero_batch_count,
        WorkloadError::ZeroExpectedParallelBatchPartitionSetCount {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            partitions: vec![0, 1],
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::Scheduler,
            [partition(1), partition(0)],
            2,
        ))
        .unwrap()
        .add_expected_parallel_batch_partition_set(expected_set(
            WorkloadParallelRemoteFlowScope::Scheduler,
            [partition(0), partition(1)],
            3,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelBatchPartitionSet {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            partitions: vec![0, 1],
        },
    );
}

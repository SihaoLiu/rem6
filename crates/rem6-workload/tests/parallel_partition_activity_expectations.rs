use rem6_boot::BootImage;
use rem6_kernel::{
    ParallelPartitionActivity, ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId,
};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelPartitionActivity, WorkloadId,
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
        rem6_workload::WorkloadManifest::builder(id("parallel-partition-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_activity(
    scope: WorkloadParallelRemoteFlowScope,
    partition: u32,
    minimum_worker_count: usize,
    minimum_dispatch_count: usize,
    minimum_remote_send_count: usize,
    minimum_remote_receive_count: usize,
) -> WorkloadExpectedParallelPartitionActivity {
    WorkloadExpectedParallelPartitionActivity::new(
        scope,
        PartitionId::new(partition),
        minimum_worker_count,
        minimum_dispatch_count,
        minimum_remote_send_count,
        minimum_remote_receive_count,
    )
    .unwrap()
}

fn expected_dma_activity(
    scope: WorkloadParallelBatchPartitionScope,
    partition: u32,
    minimum_worker_count: usize,
    minimum_dispatch_count: usize,
    minimum_remote_send_count: usize,
    minimum_remote_receive_count: usize,
) -> WorkloadExpectedParallelPartitionActivity {
    WorkloadExpectedParallelPartitionActivity::new(
        scope,
        PartitionId::new(partition),
        minimum_worker_count,
        minimum_dispatch_count,
        minimum_remote_send_count,
        minimum_remote_receive_count,
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

#[test]
fn workload_manifest_records_parallel_partition_activity_expectations() {
    let scheduler_activity =
        expected_activity(WorkloadParallelRemoteFlowScope::Scheduler, 0, 2, 3, 1, 0);
    let data_cache_activity = expected_activity(
        WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        1,
        1,
        2,
        0,
        3,
    );
    let full_system_activity =
        expected_activity(WorkloadParallelRemoteFlowScope::FullSystem, 1, 2, 5, 1, 5);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-partition-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_activity(full_system_activity)
            .unwrap()
            .add_expected_parallel_partition_activity(data_cache_activity)
            .unwrap()
            .add_expected_parallel_partition_activity(scheduler_activity)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_partition_activity(),
        &[
            scheduler_activity,
            data_cache_activity,
            full_system_activity
        ],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_partition_activity(),
        manifest.expected_parallel_partition_activity(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([
            (
                PartitionId::new(0),
                ParallelPartitionActivity::with_remote_counts(2, 3, 1, 0, 8),
            ),
            (
                PartitionId::new(1),
                ParallelPartitionActivity::with_remote_counts(1, 3, 1, 2, 4),
            ),
        ])
        .with_data_cache_parallel_scheduler_partition_activities([(
            PartitionId::new(1),
            ParallelPartitionActivity::with_remote_counts(1, 2, 0, 3, 6),
        )]);
    assert_eq!(
        summary.full_system_parallel_scheduler_partition_activity(PartitionId::new(1)),
        Some(ParallelPartitionActivity::with_remote_counts(2, 5, 1, 5, 6)),
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_partition_activity() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-partition-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let worker =
        rem6_workload::WorkloadManifest::builder(id("identity-partition-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_activity(expected_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
                1,
                0,
                0,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let remote =
        rem6_workload::WorkloadManifest::builder(id("identity-partition-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_activity(expected_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
                1,
                0,
                1,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), worker.identity());
    assert_ne!(worker.identity(), remote.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_partition_activity() {
    let plan = replay_plan()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            1,
            2,
            5,
            1,
            5,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelPartitionActivitySummary {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partition: 1,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([(
            PartitionId::new(1),
            ParallelPartitionActivity::with_remote_counts(1, 3, 1, 2, 4),
        )])
        .with_data_cache_parallel_scheduler_partition_activities([(
            PartitionId::new(1),
            ParallelPartitionActivity::with_remote_counts(0, 1, 0, 1, 6),
        )]);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedParallelPartitionActivityBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partition: 1,
            minimum_worker_count: 2,
            actual_worker_count: 1,
            minimum_dispatch_count: 5,
            actual_dispatch_count: 3,
            minimum_remote_send_count: 1,
            actual_remote_send_count: 1,
            minimum_remote_receive_count: 5,
            actual_remote_receive_count: 3,
        },
    );
}

#[test]
fn workload_replay_plan_uses_explicit_full_system_partition_activities() {
    let plan = replay_plan()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            4,
            3,
            7,
            2,
            5,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([(
            PartitionId::new(4),
            ParallelPartitionActivity::with_remote_counts(1, 2, 1, 1, 3),
        )])
        .with_full_system_parallel_scheduler_partition_activities([(
            PartitionId::new(4),
            ParallelPartitionActivity::with_remote_counts(3, 7, 2, 5, 9),
        )]);

    assert_eq!(
        summary.full_system_parallel_scheduler_partition_activity(PartitionId::new(4)),
        Some(ParallelPartitionActivity::with_remote_counts(3, 7, 2, 5, 9)),
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_ignores_workerless_partition_dispatch_activity() {
    let plan = replay_plan()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            1,
            0,
            1,
            2,
            0,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([(
            PartitionId::new(1),
            ParallelPartitionActivity::with_remote_counts(0, 3, 2, 0, 5),
        )]);

    assert_eq!(
        summary.parallel_scheduler_partition_activity(PartitionId::new(1)),
        Some(ParallelPartitionActivity::with_remote_counts(0, 0, 2, 0, 5)),
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::ExpectedParallelPartitionActivityBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            partition: 1,
            minimum_worker_count: 0,
            actual_worker_count: 0,
            minimum_dispatch_count: 1,
            actual_dispatch_count: 0,
            minimum_remote_send_count: 2,
            actual_remote_send_count: 2,
            minimum_remote_receive_count: 0,
            actual_remote_receive_count: 0,
        },
    );
}

#[test]
fn workload_replay_plan_derives_partition_remote_activity_from_flows() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("partition-activity-from-flows"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            0,
            0,
            5,
            0,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            3,
            0,
            0,
            0,
            4,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            2,
            0,
            0,
            3,
            2,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 5, 3, 17),
            ParallelRemoteFlowRecord::new(PartitionId::new(2), PartitionId::new(1), 3, 5, 13),
        ])
        .with_data_cache_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(4),
            PartitionId::new(3),
            4,
            7,
            11,
        )]);

    assert_eq!(
        summary.parallel_scheduler_partition_activity(PartitionId::new(0)),
        Some(ParallelPartitionActivity::with_remote_counts(0, 0, 5, 0, 0)),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_partition_activity(PartitionId::new(3)),
        Some(ParallelPartitionActivity::with_remote_counts(0, 0, 0, 4, 0)),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_partition_activity(PartitionId::new(2)),
        Some(ParallelPartitionActivity::with_remote_counts(0, 0, 3, 5, 0)),
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_remote_flow_partition_activity_evidence() {
    let plan = replay_plan()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            0,
            0,
            1,
            0,
        ))
        .unwrap();

    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::with_delay_bounds(
                PartitionId::new(0),
                PartitionId::new(1),
                1,
                11,
                11,
                9,
                3,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficFlowDelayBounds {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            minimum_delay: 9,
            maximum_delay: 3,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_remote_send_partition_activity_evidence() {
    let plan = replay_plan()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            0,
            0,
            1,
            0,
        ))
        .unwrap();

    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(1),
                13,
                5,
                0,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficSendTiming {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            source_tick: 13,
            delivery_tick: 5,
            order: 0,
        },
    );
}

#[test]
fn workload_replay_plan_derives_partition_activity_from_batch_partition_sets() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("partition-activity-from-batch-partition-sets"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            5,
            5,
            0,
            0,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            3,
            2,
            2,
            0,
            0,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            3,
            5,
            5,
            0,
            0,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(0),
                    PartitionId::new(2),
                    PartitionId::new(3),
                ],
                3,
            ),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(3)], 4),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(3), PartitionId::new(4)], 2),
        ]);

    assert_eq!(
        summary.parallel_scheduler_partition_activity(PartitionId::new(0)),
        Some(ParallelPartitionActivity::with_remote_counts(5, 5, 0, 0, 0)),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_partition_activity(PartitionId::new(3)),
        Some(ParallelPartitionActivity::with_remote_counts(2, 2, 0, 0, 0)),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_partition_activity(PartitionId::new(3)),
        Some(ParallelPartitionActivity::with_remote_counts(5, 5, 0, 0, 0)),
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_derives_full_system_partition_activity_from_dma_batch_timelines() {
    let plan = replay_plan()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            4,
            3,
            3,
            0,
            0,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            5,
            2,
            2,
            0,
            0,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            8,
            10,
            [PartitionId::new(3), PartitionId::new(4)],
            2,
        )])
        .with_accelerator_dma_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                10,
                12,
                [PartitionId::new(4), PartitionId::new(5)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                12,
                14,
                [PartitionId::new(5), PartitionId::new(4)],
                2,
            ),
        ]);

    assert_eq!(
        summary.full_system_parallel_scheduler_partition_activity(PartitionId::new(4)),
        Some(ParallelPartitionActivity::with_remote_counts(3, 3, 0, 0, 0)),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_partition_activity(PartitionId::new(5)),
        Some(ParallelPartitionActivity::with_remote_counts(2, 2, 0, 0, 0)),
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_verifies_direct_dma_partition_activity_from_batch_timelines() {
    let plan = replay_plan()
        .add_expected_parallel_partition_activity(expected_dma_activity(
            WorkloadParallelBatchPartitionScope::GpuDmaScheduler,
            4,
            1,
            1,
            0,
            0,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_dma_activity(
            WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler,
            5,
            2,
            2,
            0,
            0,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            8,
            10,
            [PartitionId::new(3), PartitionId::new(4)],
            2,
        )])
        .with_accelerator_dma_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                10,
                12,
                [PartitionId::new(4), PartitionId::new(5)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                12,
                14,
                [PartitionId::new(5), PartitionId::new(4)],
                2,
            ),
        ]);

    assert_eq!(
        summary.gpu_dma_scheduler_partition_activity(PartitionId::new(4)),
        Some(ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 0)),
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_partition_activity(PartitionId::new(5)),
        Some(ParallelPartitionActivity::with_remote_counts(2, 2, 0, 0, 0)),
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_malformed_timeline_for_parallel_partition_activity() {
    let plan = replay_plan()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            1,
            0,
            0,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [PartitionId::new(0), PartitionId::new(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                9,
                5,
                [PartitionId::new(0), PartitionId::new(1)],
                2,
            ),
        ]);

    assert_eq!(
        summary.parallel_scheduler_partition_activity(PartitionId::new(0)),
        Some(ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 0)),
    );
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
fn workload_replay_plan_does_not_double_count_overlapping_partition_activity_evidence() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("partition-activity-overlapping-evidence"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            5,
            7,
            3,
            2,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            0,
            4,
            4,
            0,
            0,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            0,
            9,
            11,
            3,
            2,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([(
            PartitionId::new(0),
            ParallelPartitionActivity::with_remote_counts(5, 7, 3, 2, 9),
        )])
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 3),
        ])
        .with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(1), 3, 10, 14),
            ParallelRemoteFlowRecord::new(PartitionId::new(2), PartitionId::new(0), 2, 11, 15),
        ])
        .with_data_cache_parallel_scheduler_partition_activities([(
            PartitionId::new(0),
            ParallelPartitionActivity::new(2, 2, 4),
        )])
        .with_data_cache_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(3)], 4),
        ]);

    assert_eq!(
        summary.parallel_scheduler_partition_activity(PartitionId::new(0)),
        Some(ParallelPartitionActivity::with_remote_counts(5, 7, 3, 2, 9)),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_partition_activity(PartitionId::new(0)),
        Some(ParallelPartitionActivity::with_remote_counts(4, 4, 0, 0, 4)),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_partition_activity(PartitionId::new(0)),
        Some(ParallelPartitionActivity::with_remote_counts(
            9, 11, 3, 2, 9
        )),
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_partition_activity() {
    let zero = WorkloadExpectedParallelPartitionActivity::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        PartitionId::new(0),
        0,
        0,
        0,
        0,
    )
    .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedParallelPartitionActivity {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            partition: 0,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            0,
            0,
            0,
        ))
        .unwrap()
        .add_expected_parallel_partition_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            0,
            1,
            0,
            0,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelPartitionActivity {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            partition: 0,
        },
    );
}

use rem6_boot::BootImage;
use rem6_kernel::{
    ParallelPartitionActivity, ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId,
};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelPartitionUse, WorkloadId,
    WorkloadParallelBatchPartitionScope, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord, WorkloadParallelExecutionSummary,
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

fn expected_partitions(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_active_partitions: usize,
) -> WorkloadExpectedParallelPartitionUse {
    WorkloadExpectedParallelPartitionUse::new(scope, minimum_active_partitions).unwrap()
}

fn expected_dma_partitions(
    scope: WorkloadParallelBatchPartitionScope,
    minimum_active_partitions: usize,
) -> WorkloadExpectedParallelPartitionUse {
    WorkloadExpectedParallelPartitionUse::new(scope, minimum_active_partitions).unwrap()
}

fn replay_plan() -> WorkloadReplayPlan {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-partition-use"), boot_image())
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
fn workload_manifest_records_parallel_partition_expectations() {
    let scheduler_partitions = expected_partitions(WorkloadParallelRemoteFlowScope::Scheduler, 2);
    let data_cache_partitions =
        expected_partitions(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3);
    let full_system_partitions =
        expected_partitions(WorkloadParallelRemoteFlowScope::FullSystem, 4);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(full_system_partitions)
            .unwrap()
            .add_expected_parallel_partition_use(data_cache_partitions)
            .unwrap()
            .add_expected_parallel_partition_use(scheduler_partitions)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_partition_use(),
        &[
            scheduler_partitions,
            data_cache_partitions,
            full_system_partitions,
        ],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_partition_use(),
        manifest.expected_parallel_partition_use(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_partitions(2, 2)
        .with_data_cache_parallel_counts(1, 1, 2, 1, 3)
        .with_data_cache_parallel_partitions(3)
        .with_full_system_parallel_partitions(4);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_partition_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::Scheduler,
                3,
            ))
            .unwrap()
            .build()
            .unwrap();
    let full_system =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-partitions"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::FullSystem,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger_scheduler.identity());
    assert_ne!(scheduler.identity(), full_system.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underused_parallel_partitions() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-partitions-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelPartitionSummary {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            minimum_active_partitions: 2,
        },
    );

    let single_partition_summary =
        WorkloadParallelExecutionSummary::default().with_scheduler_partitions(1, 1);
    let single_partition_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(single_partition_summary);
    assert_eq!(
        plan.verify_result(&single_partition_result).unwrap_err(),
        WorkloadError::ExpectedParallelPartitionCountBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            minimum_active_partitions: 2,
            actual_active_partitions: 1,
        },
    );
}

#[test]
fn workload_replay_plan_derives_full_system_partition_use_from_activity() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-from-activity"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::FullSystem,
            3,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([
            (
                PartitionId::new(0),
                ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 1),
            ),
            (
                PartitionId::new(1),
                ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 1),
            ),
        ])
        .with_data_cache_parallel_scheduler_partition_activities([
            (
                PartitionId::new(1),
                ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 1),
            ),
            (
                PartitionId::new(2),
                ParallelPartitionActivity::with_remote_counts(1, 1, 0, 0, 1),
            ),
        ]);

    assert_eq!(
        summary.active_full_system_parallel_scheduler_partition_count(),
        3
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_derives_active_partitions_from_remote_flows() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-from-flows"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            3,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::FullSystem,
            5,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 5, 3, 17),
            ParallelRemoteFlowRecord::new(PartitionId::new(2), PartitionId::new(1), 3, 5, 13),
        ])
        .with_data_cache_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(3),
            PartitionId::new(4),
            4,
            7,
            11,
        )]);

    assert_eq!(summary.active_scheduler_partition_count(), 3);
    assert_eq!(
        summary.active_data_cache_parallel_scheduler_partition_count(),
        2,
    );
    assert_eq!(
        summary.active_full_system_parallel_scheduler_partition_count(),
        5,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_derives_active_partitions_from_remote_sends() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-from-sends"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            3,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::FullSystem,
            5,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(2),
                3,
                9,
                0,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(2),
                PartitionId::new(1),
                5,
                13,
                1,
            ),
        ])
        .with_data_cache_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(3),
            PartitionId::new(4),
            7,
            11,
            0,
        )]);

    assert_eq!(summary.active_scheduler_partition_count(), 3);
    assert_eq!(
        summary.active_data_cache_parallel_scheduler_partition_count(),
        2,
    );
    assert_eq!(
        summary.active_full_system_parallel_scheduler_partition_count(),
        5,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_remote_send_partition_use_evidence() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-invalid-remote-send"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
        ))
        .unwrap();

    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(1),
                3,
                11,
                0,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(2),
                PartitionId::new(2),
                5,
                13,
                1,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficSendEndpoints {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 2,
            target: 2,
            source_tick: 5,
            delivery_tick: 13,
            order: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_remote_flow_partition_use_evidence() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-invalid-remote-flow"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
        ))
        .unwrap();

    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(1), 1, 13, 5),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficFlowTiming {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            send_count: 1,
            first_tick: 13,
            last_tick: 5,
        },
    );
}

#[test]
fn workload_replay_plan_derives_active_partitions_from_batch_partition_sets() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-from-batch-partition-sets"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            5,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            4,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::FullSystem,
            8,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(2),
                    PartitionId::new(3),
                    PartitionId::new(4),
                ],
                1,
            ),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(4), PartitionId::new(5)], 3),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(6), PartitionId::new(7)], 1),
        ]);

    assert_eq!(summary.active_scheduler_partition_count(), 5);
    assert_eq!(
        summary.active_data_cache_parallel_scheduler_partition_count(),
        4,
    );
    assert_eq!(
        summary.active_full_system_parallel_scheduler_partition_count(),
        8,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_ignores_single_partition_batch_sets_for_partition_use() {
    let plan = replay_plan()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0)], 1),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(1)], 1),
        ]);

    assert_eq!(summary.active_scheduler_partition_count(), 0);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::ExpectedParallelPartitionCountBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            minimum_active_partitions: 2,
            actual_active_partitions: 0,
        },
    );
}

#[test]
fn workload_replay_plan_ignores_single_partition_batch_streaks_for_partition_use() {
    let plan = replay_plan()
        .add_expected_parallel_partition_use(expected_partitions(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_streaks([
            WorkloadParallelBatchPartitionStreak::new([PartitionId::new(0)], 1),
            WorkloadParallelBatchPartitionStreak::new([PartitionId::new(1)], 1),
        ]);

    assert_eq!(summary.active_scheduler_partition_count(), 0);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::ExpectedParallelPartitionCountBelowMinimum {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            minimum_active_partitions: 2,
            actual_active_partitions: 0,
        },
    );
}

#[test]
fn workload_replay_plan_verifies_direct_dma_active_partitions_from_batch_timelines() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-partitions-from-dma-timelines"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_partition_use(expected_dma_partitions(
            WorkloadParallelBatchPartitionScope::GpuDmaScheduler,
            2,
        ))
        .unwrap()
        .add_expected_parallel_partition_use(expected_dma_partitions(
            WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler,
            3,
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
                [PartitionId::new(5), PartitionId::new(6)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                12,
                14,
                [PartitionId::new(6), PartitionId::new(7)],
                2,
            ),
        ]);

    assert_eq!(summary.active_gpu_dma_scheduler_partition_count(), 2);
    assert_eq!(
        summary.active_accelerator_dma_scheduler_partition_count(),
        3,
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_parallel_partition_expectations() {
    let zero =
        WorkloadExpectedParallelPartitionUse::new(WorkloadParallelRemoteFlowScope::FullSystem, 0)
            .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedParallelPartitionCount {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
        },
    );
    let serial =
        WorkloadExpectedParallelPartitionUse::new(WorkloadParallelRemoteFlowScope::Scheduler, 1)
            .unwrap_err();
    assert_eq!(
        serial,
        WorkloadError::InvalidExpectedParallelPartitionCount {
            scope: WorkloadParallelBatchPartitionScope::Scheduler,
            minimum_active_partitions: 1,
        },
    );

    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-partitions-duplicate"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::FullSystem,
                2,
            ))
            .unwrap()
            .add_expected_parallel_partition_use(expected_partitions(
                WorkloadParallelRemoteFlowScope::FullSystem,
                3,
            ))
            .unwrap_err();
    assert_eq!(
        manifest,
        WorkloadError::DuplicateExpectedParallelPartitionUse {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
        },
    );
}

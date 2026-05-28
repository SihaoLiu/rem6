use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchWorkerBucket,
    WorkloadExpectedParallelBatchWorkerTickActivity, WorkloadExpectedParallelBatchWorkerTickBucket,
    WorkloadExpectedParallelBatchWorkerTickStreak, WorkloadExpectedParallelBatchWorkerTicks,
    WorkloadExpectedParallelWorkerActivity, WorkloadExpectedParallelWorkerUse,
    WorkloadExpectedPlannedParallelBatchIdleWorkerTicks,
    WorkloadExpectedPlannedParallelBatchUtilization,
    WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks,
    WorkloadExpectedPlannedParallelBatchWorkerSlotTicks, WorkloadId, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord, WorkloadParallelBatchWorkerLaneRecord,
    WorkloadParallelBatchWorkerScope, WorkloadParallelExecutionSummary,
    WorkloadPlannedParallelBatchIdleExpectationError,
    WorkloadPlannedParallelBatchUtilizationExpectationError,
    WorkloadPlannedParallelBatchWorkerLanePartitionExpectationError,
    WorkloadPlannedParallelBatchWorkerSlotExpectationError, WorkloadReplayPlan, WorkloadResource,
    WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
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
        rem6_workload::WorkloadManifest::builder(id("planned-batch-workers"), boot_image())
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

fn planned_lane_record(
    scope: WorkloadParallelBatchScope,
    lane: usize,
    partition: PartitionId,
    start_tick: u64,
    horizon: u64,
) -> WorkloadParallelBatchWorkerLaneRecord {
    WorkloadParallelBatchWorkerLaneRecord::new(scope, lane, partition, start_tick, horizon)
}

#[test]
fn workload_replay_plan_checks_planned_parallel_batch_utilization_directly() {
    let plan = replay_plan()
        .add_expected_planned_parallel_batch_utilization(
            WorkloadExpectedPlannedParallelBatchUtilization::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                3,
                4,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2)],
            1,
        )])
        .with_full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(16);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let underutilized = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2)],
            1,
        )])
        .with_full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(20);
    let underutilized_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underutilized);

    assert_eq!(
        plan.verify_result(&underutilized_result).unwrap_err(),
        WorkloadError::PlannedParallelBatchUtilizationExpectation(
            WorkloadPlannedParallelBatchUtilizationExpectationError::BelowMinimum {
                scope: WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                minimum_numerator: 3,
                minimum_denominator: 4,
                actual_numerator: 12,
                actual_denominator: 20,
            },
        ),
    );
}

#[test]
fn workload_manifest_carries_planned_parallel_batch_utilization_expectation() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("planned-utilization-manifest"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_planned_parallel_batch_utilization(
                WorkloadExpectedPlannedParallelBatchUtilization::new(
                    WorkloadParallelBatchWorkerScope::PlannedScheduler,
                    1,
                    2,
                )
                .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            8,
            [partition(0), partition(1)],
            2,
        )])
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(16);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(plan.expected_planned_parallel_batch_utilization().len(), 1);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_checks_planned_parallel_batch_idle_budget() {
    let plan = replay_plan()
        .add_expected_planned_parallel_batch_idle_worker_ticks(
            WorkloadExpectedPlannedParallelBatchIdleWorkerTicks::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                4,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(10)
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(10);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let idle_heavy = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(12)
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(12);
    let idle_heavy_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(idle_heavy);

    assert_eq!(
        plan.verify_result(&idle_heavy_result).unwrap_err(),
        WorkloadError::PlannedParallelBatchIdleExpectation(
            WorkloadPlannedParallelBatchIdleExpectationError::AboveMaximum {
                scope: WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                maximum_idle_worker_ticks: 4,
                actual_idle_worker_ticks: 8,
            },
        ),
    );
}

#[test]
fn workload_manifest_carries_planned_parallel_batch_idle_budget() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("planned-idle-manifest"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_planned_parallel_batch_idle_worker_ticks(
                WorkloadExpectedPlannedParallelBatchIdleWorkerTicks::new(
                    WorkloadParallelBatchWorkerScope::PlannedScheduler,
                    0,
                )
                .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            8,
            [partition(0), partition(1)],
            2,
        )])
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(16);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.expected_planned_parallel_batch_idle_worker_ticks()
            .len(),
        1
    );
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_checks_planned_parallel_batch_worker_slot_ticks() {
    let plan = replay_plan()
        .add_expected_planned_parallel_batch_worker_slot_ticks(
            WorkloadExpectedPlannedParallelBatchWorkerSlotTicks::new(
                WorkloadParallelBatchWorkerScope::PlannedScheduler,
                1,
                6,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                6,
                [partition(0), partition(1)],
                2,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                6,
                8,
                [partition(0)],
                1,
            ),
        ])
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(16);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        result
            .parallel_execution_summary()
            .unwrap()
            .parallel_scheduler_planned_batch_worker_slot_tick_summaries(),
        vec![(0, 8, 0), (1, 6, 2)],
    );
    plan.verify_result(&result).unwrap();

    let underactive = WorkloadParallelExecutionSummary::default()
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
                [partition(0)],
                1,
            ),
        ])
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(16);
    let underactive_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive);

    assert_eq!(
        plan.verify_result(&underactive_result).unwrap_err(),
        WorkloadError::PlannedParallelBatchWorkerSlotExpectation(
            WorkloadPlannedParallelBatchWorkerSlotExpectationError::BelowMinimumActive {
                scope: WorkloadParallelBatchWorkerScope::PlannedScheduler,
                worker_slot: 1,
                minimum_active_ticks: 6,
                actual_active_ticks: 4,
            },
        ),
    );

    let idle_plan = replay_plan()
        .add_expected_planned_parallel_batch_worker_slot_ticks(
            WorkloadExpectedPlannedParallelBatchWorkerSlotTicks::new(
                WorkloadParallelBatchWorkerScope::PlannedScheduler,
                1,
                0,
                1,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        idle_plan.verify_result(&underactive_result).unwrap_err(),
        WorkloadError::PlannedParallelBatchWorkerSlotExpectation(
            WorkloadPlannedParallelBatchWorkerSlotExpectationError::AboveMaximumIdle {
                scope: WorkloadParallelBatchWorkerScope::PlannedScheduler,
                worker_slot: 1,
                maximum_idle_ticks: 1,
                actual_idle_ticks: 4,
            },
        ),
    );
}

#[test]
fn workload_summary_preserves_planned_parallel_batch_worker_lane_records() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_worker_lanes([
            planned_lane_record(WorkloadParallelBatchScope::Scheduler, 1, partition(1), 2, 8),
            planned_lane_record(WorkloadParallelBatchScope::Scheduler, 0, partition(0), 0, 8),
        ])
        .with_data_cache_parallel_scheduler_planned_batch_worker_lanes([planned_lane_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            0,
            partition(2),
            8,
            12,
        )]);

    assert_eq!(
        summary.parallel_scheduler_planned_batch_worker_lanes(),
        &[
            planned_lane_record(WorkloadParallelBatchScope::Scheduler, 0, partition(0), 0, 8,),
            planned_lane_record(WorkloadParallelBatchScope::Scheduler, 1, partition(1), 2, 8,),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_worker_lanes(),
        vec![
            planned_lane_record(WorkloadParallelBatchScope::Scheduler, 0, partition(0), 0, 8,),
            planned_lane_record(WorkloadParallelBatchScope::Scheduler, 1, partition(1), 2, 8,),
            planned_lane_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                0,
                partition(2),
                8,
                12,
            ),
        ],
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_worker_lane_tick_summaries(),
        vec![(0, 8), (1, 6)],
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_worker_lane_partition_ticks(1, partition(1)),
        6,
    );
}

#[test]
fn workload_replay_plan_checks_planned_parallel_batch_worker_lane_partition_ticks() {
    let plan = replay_plan()
        .add_expected_planned_parallel_batch_worker_lane_partition_ticks(
            WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                1,
                partition(1),
                6,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_worker_lanes([
            planned_lane_record(WorkloadParallelBatchScope::Scheduler, 0, partition(0), 0, 8),
            planned_lane_record(WorkloadParallelBatchScope::Scheduler, 1, partition(1), 2, 8),
        ])
        .with_data_cache_parallel_scheduler_planned_batch_worker_lanes([planned_lane_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            0,
            partition(2),
            8,
            12,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let wrong_partition = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_worker_lanes([planned_lane_record(
            WorkloadParallelBatchScope::Scheduler,
            1,
            partition(2),
            2,
            8,
        )]);
    let wrong_partition_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(wrong_partition);

    assert_eq!(
        plan.verify_result(&wrong_partition_result).unwrap_err(),
        WorkloadError::PlannedParallelBatchWorkerLanePartitionExpectation(
            WorkloadPlannedParallelBatchWorkerLanePartitionExpectationError::BelowMinimum {
                scope: WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                worker_lane: 1,
                partition: partition(1),
                minimum_ticks: 6,
                actual_ticks: 0,
            },
        ),
    );
}

#[test]
fn workload_planned_parallel_batch_worker_lane_partition_ticks_reject_zero_minimum() {
    let error = WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks::new(
        WorkloadParallelBatchWorkerScope::PlannedScheduler,
        0,
        partition(0),
        0,
    )
    .unwrap_err();

    assert_eq!(
        error,
        WorkloadError::PlannedParallelBatchWorkerLanePartitionExpectation(
            WorkloadPlannedParallelBatchWorkerLanePartitionExpectationError::ZeroMinimumTicks {
                scope: WorkloadParallelBatchWorkerScope::PlannedScheduler,
                worker_lane: 0,
                partition: partition(0),
            },
        ),
    );
}

#[test]
fn workload_manifest_carries_planned_parallel_batch_worker_lane_partition_ticks() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("planned-worker-lane-manifest"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_planned_parallel_batch_worker_lane_partition_ticks(
                WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks::new(
                    WorkloadParallelBatchWorkerScope::PlannedScheduler,
                    0,
                    partition(0),
                    8,
                )
                .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_worker_lanes([planned_lane_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            partition(0),
            0,
            8,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.expected_planned_parallel_batch_worker_lane_partition_ticks()
            .len(),
        1
    );
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_carries_planned_parallel_batch_worker_slot_ticks() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("planned-worker-slot-manifest"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_planned_parallel_batch_worker_slot_ticks(
                WorkloadExpectedPlannedParallelBatchWorkerSlotTicks::new(
                    WorkloadParallelBatchWorkerScope::PlannedScheduler,
                    0,
                    8,
                    0,
                )
                .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            8,
            [partition(0), partition(1)],
            2,
        )])
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(16);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.expected_planned_parallel_batch_worker_slot_ticks()
            .len(),
        1
    );
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_checks_planned_parallel_batch_worker_expectations_directly() {
    let plan = replay_plan()
        .add_expected_parallel_batch_worker_bucket(
            WorkloadExpectedParallelBatchWorkerBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedScheduler,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_bucket(
            WorkloadExpectedParallelBatchWorkerBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_tick_bucket(
            WorkloadExpectedParallelBatchWorkerTickBucket::new(
                WorkloadParallelBatchWorkerScope::PlannedScheduler,
                2,
                4,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_tick_activity(
            WorkloadExpectedParallelBatchWorkerTickActivity::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                2,
                8,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_tick_streak(
            WorkloadExpectedParallelBatchWorkerTickStreak::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                2,
                8,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_batch_worker_ticks(
            WorkloadExpectedParallelBatchWorkerTicks::new_at_or_above(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                2,
                16,
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
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let actual_only = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )]);
    let missing_planned = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_only);

    assert_eq!(
        plan.verify_result(&missing_planned).unwrap_err(),
        WorkloadError::ExpectedParallelBatchWorkerBucketBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::PlannedScheduler,
            worker_count: 2,
            minimum_batch_count: 1,
            actual_batch_count: 0,
        },
    );
}

#[test]
fn workload_replay_plan_checks_planned_parallel_worker_use_and_activity() {
    let plan = replay_plan()
        .add_expected_parallel_worker_use(
            WorkloadExpectedParallelWorkerUse::new(
                WorkloadParallelBatchWorkerScope::PlannedScheduler,
                2,
            )
            .unwrap(),
        )
        .unwrap()
        .add_expected_parallel_worker_activity(
            WorkloadExpectedParallelWorkerActivity::new(
                WorkloadParallelBatchWorkerScope::PlannedFullSystem,
                4,
            )
            .unwrap(),
        )
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0)],
            1,
        )])
        .with_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )])
        .with_data_cache_parallel_scheduler_planned_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [partition(2), partition(3)],
            2,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();

    let actual_only = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0), partition(1)],
            2,
        )]);
    let missing_planned = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(actual_only);

    assert_eq!(
        plan.verify_result(&missing_planned).unwrap_err(),
        WorkloadError::ExpectedParallelWorkerCountBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::PlannedScheduler,
            minimum_max_workers: 2,
            actual_max_workers: 0,
        },
    );
}

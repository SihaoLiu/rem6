use rem6_kernel::{ParallelPartitionActivity, PartitionId};

use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelSchedulerProgress, WorkloadId,
    WorkloadParallelBatchPartitionSet, WorkloadParallelBatchWorkerCount,
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
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-scheduler-progress"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_progress(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_epoch_count: usize,
    minimum_dispatch_count: usize,
) -> WorkloadExpectedParallelSchedulerProgress {
    WorkloadExpectedParallelSchedulerProgress::new(
        scope,
        minimum_epoch_count,
        minimum_dispatch_count,
    )
    .unwrap()
}

#[test]
fn workload_manifest_records_parallel_scheduler_progress_expectations() {
    let scheduler_progress = expected_progress(WorkloadParallelRemoteFlowScope::Scheduler, 2, 5);
    let data_cache_progress =
        expected_progress(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3, 7);
    let full_system_progress =
        expected_progress(WorkloadParallelRemoteFlowScope::FullSystem, 5, 12);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-scheduler-progress"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_scheduler_progress(full_system_progress)
            .unwrap()
            .add_expected_parallel_scheduler_progress(data_cache_progress)
            .unwrap()
            .add_expected_parallel_scheduler_progress(scheduler_progress)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_scheduler_progress(),
        &[
            scheduler_progress,
            data_cache_progress,
            full_system_progress
        ],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_scheduler_progress(),
        manifest.expected_parallel_scheduler_progress(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(2, 0, 5, 2)
        .with_data_cache_parallel_counts(1, 3, 7, 4, 2);
    assert_eq!(summary.full_system_parallel_scheduler_epoch_count(), 5);
    assert_eq!(summary.full_system_parallel_scheduler_dispatch_count(), 12);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_scheduler_progress() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-scheduler-progress"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-scheduler-progress"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_scheduler_progress(expected_progress(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
                5,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger =
        rem6_workload::WorkloadManifest::builder(id("identity-scheduler-progress"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_scheduler_progress(expected_progress(
                WorkloadParallelRemoteFlowScope::Scheduler,
                3,
                5,
            ))
            .unwrap()
            .build()
            .unwrap();
    let wider =
        rem6_workload::WorkloadManifest::builder(id("identity-scheduler-progress"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_scheduler_progress(expected_progress(
                WorkloadParallelRemoteFlowScope::FullSystem,
                2,
                5,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger.identity());
    assert_ne!(scheduler.identity(), wider.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_parallel_scheduler_progress() {
    let plan = replay_plan()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::FullSystem,
            5,
            12,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelSchedulerProgressSummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_epoch_count: 5,
            minimum_dispatch_count: 12,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(2, 0, 5, 2)
        .with_data_cache_parallel_counts(1, 2, 6, 4, 2);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedParallelSchedulerProgressBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_epoch_count: 5,
            actual_epoch_count: 4,
            minimum_dispatch_count: 12,
            actual_dispatch_count: 11,
        },
    );
}

#[test]
fn workload_replay_plan_derives_dispatch_progress_from_partition_activity() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("scheduler-progress-from-partition-activity"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            8,
        ))
        .unwrap()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            0,
            7,
        ))
        .unwrap()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::FullSystem,
            0,
            15,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([
            (PartitionId::new(0), ParallelPartitionActivity::new(1, 5, 8)),
            (PartitionId::new(1), ParallelPartitionActivity::new(1, 3, 4)),
        ])
        .with_data_cache_parallel_scheduler_partition_activities([
            (
                PartitionId::new(10),
                ParallelPartitionActivity::new(1, 4, 6),
            ),
            (
                PartitionId::new(11),
                ParallelPartitionActivity::new(1, 3, 3),
            ),
        ]);

    assert_eq!(summary.scheduler_dispatch_count(), 8);
    assert_eq!(summary.data_cache_parallel_scheduler_dispatch_count(), 7);
    assert_eq!(summary.full_system_parallel_scheduler_dispatch_count(), 15);
    assert!(summary.has_parallel_scheduler_work());
    assert!(summary.has_data_cache_parallel_work());
    assert!(summary.has_full_system_parallel_scheduler_work());
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_derives_dispatch_progress_from_batch_evidence() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("scheduler-progress-from-batch-evidence"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            10,
        ))
        .unwrap()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            0,
            11,
        ))
        .unwrap()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::FullSystem,
            0,
            21,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(2, 3),
            WorkloadParallelBatchWorkerCount::new(4, 1),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(10), PartitionId::new(11)], 3),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(10),
                    PartitionId::new(12),
                    PartitionId::new(13),
                    PartitionId::new(14),
                    PartitionId::new(15),
                ],
                1,
            ),
        ]);

    assert_eq!(summary.scheduler_dispatch_count(), 10);
    assert_eq!(summary.data_cache_parallel_scheduler_dispatch_count(), 11);
    assert_eq!(summary.full_system_parallel_scheduler_dispatch_count(), 21);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_scheduler_progress() {
    let zero = WorkloadExpectedParallelSchedulerProgress::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        0,
        0,
    )
    .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedParallelSchedulerProgress {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            3,
            7,
        ))
        .unwrap()
        .add_expected_parallel_scheduler_progress(expected_progress(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            4,
            8,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelSchedulerProgress {
            scope: WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        },
    );
}

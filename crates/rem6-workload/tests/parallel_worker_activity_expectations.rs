use rem6_boot::BootImage;
use rem6_kernel::{ParallelPartitionActivity, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelWorkerActivity, WorkloadId,
    WorkloadParallelBatchWorkerCount, WorkloadParallelExecutionSummary,
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
        rem6_workload::WorkloadManifest::builder(id("parallel-worker-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_activity(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_total_workers: usize,
) -> WorkloadExpectedParallelWorkerActivity {
    WorkloadExpectedParallelWorkerActivity::new(scope, minimum_total_workers).unwrap()
}

#[test]
fn workload_manifest_records_parallel_worker_activity_expectations() {
    let scheduler_activity = expected_activity(WorkloadParallelRemoteFlowScope::Scheduler, 5);
    let data_cache_activity =
        expected_activity(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 4);
    let full_system_activity = expected_activity(WorkloadParallelRemoteFlowScope::FullSystem, 9);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-worker-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_activity(full_system_activity)
            .unwrap()
            .add_expected_parallel_worker_activity(data_cache_activity)
            .unwrap()
            .add_expected_parallel_worker_activity(scheduler_activity)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_worker_activity(),
        &[
            scheduler_activity,
            data_cache_activity,
            full_system_activity
        ],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_worker_activity(),
        manifest.expected_parallel_worker_activity(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_worker_count(5)
        .with_scheduler_partitions(2, 2)
        .with_data_cache_parallel_worker_count(4)
        .with_data_cache_parallel_counts(1, 2, 3, 2, 3);
    assert_eq!(summary.full_system_parallel_scheduler_total_workers(), 9);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_worker_activity() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_activity(expected_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                5,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_activity(expected_activity(
                WorkloadParallelRemoteFlowScope::Scheduler,
                6,
            ))
            .unwrap()
            .build()
            .unwrap();
    let full_system =
        rem6_workload::WorkloadManifest::builder(id("identity-worker-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_activity(expected_activity(
                WorkloadParallelRemoteFlowScope::FullSystem,
                5,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stronger_scheduler.identity());
    assert_ne!(scheduler.identity(), full_system.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_parallel_workers() {
    let plan = replay_plan()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            8,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelWorkerActivitySummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_total_workers: 8,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_worker_count(3)
        .with_data_cache_parallel_worker_count(4);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedParallelWorkerActivityBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            minimum_total_workers: 8,
            actual_total_workers: 7,
        },
    );
}

#[test]
fn workload_replay_plan_derives_total_workers_from_batch_histograms() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-worker-activity-from-batches"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            10,
        ))
        .unwrap()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            11,
        ))
        .unwrap()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            21,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(2, 3),
            WorkloadParallelBatchWorkerCount::new(4, 1),
        ])
        .with_data_cache_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(3, 2),
            WorkloadParallelBatchWorkerCount::new(5, 1),
        ]);

    assert_eq!(summary.total_parallel_scheduler_workers(), 10);
    assert_eq!(summary.data_cache_parallel_scheduler_total_workers(), 11);
    assert_eq!(summary.full_system_parallel_scheduler_total_workers(), 21);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_derives_total_workers_from_partition_activity() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-worker-activity-from-partitions"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::Scheduler,
            5,
        ))
        .unwrap()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            7,
        ))
        .unwrap()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::FullSystem,
            12,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_partition_activities([
            (PartitionId::new(0), ParallelPartitionActivity::new(2, 3, 5)),
            (PartitionId::new(1), ParallelPartitionActivity::new(3, 1, 2)),
        ])
        .with_data_cache_parallel_scheduler_partition_activities([
            (PartitionId::new(2), ParallelPartitionActivity::new(4, 2, 7)),
            (PartitionId::new(3), ParallelPartitionActivity::new(3, 2, 4)),
        ]);

    assert_eq!(summary.total_parallel_scheduler_workers(), 5);
    assert_eq!(summary.data_cache_parallel_scheduler_total_workers(), 7);
    assert_eq!(summary.full_system_parallel_scheduler_total_workers(), 12);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_worker_activity() {
    let zero =
        WorkloadExpectedParallelWorkerActivity::new(WorkloadParallelRemoteFlowScope::Scheduler, 0)
            .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedParallelWorkerActivity {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            5,
        ))
        .unwrap()
        .add_expected_parallel_worker_activity(expected_activity(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            6,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelWorkerActivity {
            scope: WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        },
    );
}

use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelBatchTimelineRecord, WorkloadId,
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
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
    let manifest = rem6_workload::WorkloadManifest::builder(id("batch-timeline"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_timeline(
    scope: WorkloadParallelRemoteFlowScope,
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
        WorkloadParallelRemoteFlowScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(1), partition(0)],
        2,
    );
    let data_cache = expected_timeline(
        WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        WorkloadParallelBatchScope::DataCacheScheduler,
        4,
        8,
        [partition(2)],
        1,
    );
    let full_system_scheduler = expected_timeline(
        WorkloadParallelRemoteFlowScope::FullSystem,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0), partition(1)],
        2,
    );
    let full_system_data_cache = expected_timeline(
        WorkloadParallelRemoteFlowScope::FullSystem,
        WorkloadParallelBatchScope::DataCacheScheduler,
        4,
        8,
        [partition(2)],
        1,
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
        WorkloadParallelRemoteFlowScope::Scheduler
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
            [partition(2)],
            1,
        )]);
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
                WorkloadParallelRemoteFlowScope::Scheduler,
                WorkloadParallelBatchScope::Scheduler,
                4,
                8,
                [partition(0)],
                1,
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
                WorkloadParallelRemoteFlowScope::Scheduler,
                WorkloadParallelBatchScope::Scheduler,
                8,
                12,
                [partition(0)],
                1,
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
            WorkloadParallelRemoteFlowScope::Scheduler,
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0)],
            1,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelBatchTimelineSummary {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0],
            worker_count: 1,
        },
    );

    let wrong_timeline = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            4,
            8,
            [partition(0)],
            1,
        )]);
    let wrong = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(wrong_timeline);
    assert_eq!(
        plan.verify_result(&wrong).unwrap_err(),
        WorkloadError::ExpectedParallelBatchTimelineRecordMissing {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0],
            worker_count: 1,
        },
    );

    let extra_timeline = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [partition(0)],
                1,
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
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
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
            WorkloadParallelRemoteFlowScope::Scheduler,
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [partition(0)],
            1,
        ))
        .unwrap();
    let duplicate_timeline = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [partition(0)],
                1,
            ),
            timeline_record(
                WorkloadParallelBatchScope::Scheduler,
                0,
                4,
                [partition(0)],
                1,
            ),
        ]);
    let duplicate = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(duplicate_timeline);

    assert_eq!(
        plan.verify_result(&duplicate).unwrap_err(),
        WorkloadError::UnexpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0],
            worker_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_batch_timeline_records() {
    let zero_worker = WorkloadExpectedParallelBatchTimelineRecord::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0)],
        0,
    );
    assert_eq!(
        zero_worker.unwrap_err(),
        WorkloadError::InvalidExpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0],
            worker_count: 0,
        },
    );

    let expected = expected_timeline(
        WorkloadParallelRemoteFlowScope::Scheduler,
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [partition(0)],
        1,
    );
    assert_eq!(
        replay_plan()
            .add_expected_parallel_batch_timeline_record(expected.clone())
            .unwrap()
            .add_expected_parallel_batch_timeline_record(expected)
            .unwrap_err(),
        WorkloadError::DuplicateExpectedParallelBatchTimelineRecord {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            batch_scope: WorkloadParallelBatchScope::Scheduler,
            start_tick: 0,
            horizon: 4,
            partitions: vec![0],
            worker_count: 1,
        },
    );
}

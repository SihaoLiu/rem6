use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelSchedulerIdleBound, WorkloadId,
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
        rem6_workload::WorkloadManifest::builder(id("parallel-scheduler-idle-bound"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn idle_bound(
    scope: WorkloadParallelRemoteFlowScope,
    maximum_empty_epoch_count: usize,
) -> WorkloadExpectedParallelSchedulerIdleBound {
    WorkloadExpectedParallelSchedulerIdleBound::new(scope, maximum_empty_epoch_count)
}

#[test]
fn workload_manifest_records_parallel_scheduler_idle_bounds() {
    let scheduler_bound = idle_bound(WorkloadParallelRemoteFlowScope::Scheduler, 1);
    let data_cache_bound = idle_bound(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 2);
    let full_system_bound = idle_bound(WorkloadParallelRemoteFlowScope::FullSystem, 3);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-scheduler-idle"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_scheduler_idle_bound(full_system_bound)
            .unwrap()
            .add_expected_parallel_scheduler_idle_bound(data_cache_bound)
            .unwrap()
            .add_expected_parallel_scheduler_idle_bound(scheduler_bound)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_scheduler_idle_bounds(),
        &[scheduler_bound, data_cache_bound, full_system_bound],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_scheduler_idle_bounds(),
        manifest.expected_parallel_scheduler_idle_bounds(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(3, 1, 7, 5)
        .with_data_cache_parallel_counts(1, 4, 9, 8, 3)
        .with_data_cache_parallel_empty_epoch_count(2);
    assert_eq!(
        summary.full_system_parallel_scheduler_empty_epoch_count(),
        3
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_scheduler_idle_bounds() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-scheduler-idle"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-scheduler-idle"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_scheduler_idle_bound(idle_bound(
                WorkloadParallelRemoteFlowScope::Scheduler,
                1,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stricter =
        rem6_workload::WorkloadManifest::builder(id("identity-scheduler-idle"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_scheduler_idle_bound(idle_bound(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let wider =
        rem6_workload::WorkloadManifest::builder(id("identity-scheduler-idle"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_scheduler_idle_bound(idle_bound(
                WorkloadParallelRemoteFlowScope::FullSystem,
                1,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), stricter.identity());
    assert_ne!(scheduler.identity(), wider.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_overidle_parallel_scheduler() {
    let plan = replay_plan()
        .add_expected_parallel_scheduler_idle_bound(idle_bound(
            WorkloadParallelRemoteFlowScope::FullSystem,
            2,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelSchedulerIdleSummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            maximum_empty_epoch_count: 2,
        },
    );

    let overidle_summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(5, 2, 5, 2)
        .with_data_cache_parallel_counts(1, 4, 7, 4, 2)
        .with_data_cache_parallel_empty_epoch_count(1);
    let overidle = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(overidle_summary);
    assert_eq!(
        plan.verify_result(&overidle).unwrap_err(),
        WorkloadError::ExpectedParallelSchedulerIdleAboveMaximum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            maximum_empty_epoch_count: 2,
            actual_empty_epoch_count: 3,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_parallel_scheduler_idle_bounds() {
    let duplicate = replay_plan()
        .add_expected_parallel_scheduler_idle_bound(idle_bound(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
        ))
        .unwrap()
        .add_expected_parallel_scheduler_idle_bound(idle_bound(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            1,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelSchedulerIdleBound {
            scope: WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        },
    );
}

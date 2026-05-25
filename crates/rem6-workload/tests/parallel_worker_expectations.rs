use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelWorkerUse, WorkloadId, WorkloadParallelExecutionSummary,
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

fn expected_workers(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_max_workers: usize,
) -> WorkloadExpectedParallelWorkerUse {
    WorkloadExpectedParallelWorkerUse::new(scope, minimum_max_workers).unwrap()
}

#[test]
fn workload_manifest_records_parallel_worker_expectations() {
    let scheduler_workers = expected_workers(WorkloadParallelRemoteFlowScope::Scheduler, 2);
    let data_cache_workers =
        expected_workers(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 3);
    let full_system_workers = expected_workers(WorkloadParallelRemoteFlowScope::FullSystem, 3);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-parallel-workers"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_use(full_system_workers)
            .unwrap()
            .add_expected_parallel_worker_use(data_cache_workers)
            .unwrap()
            .add_expected_parallel_worker_use(scheduler_workers)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_worker_use(),
        &[scheduler_workers, data_cache_workers, full_system_workers],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_worker_use(),
        manifest.expected_parallel_worker_use(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_partitions(3, 2)
        .with_data_cache_parallel_counts(1, 1, 2, 1, 3);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_worker_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-workers"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-workers"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_use(expected_workers(
                WorkloadParallelRemoteFlowScope::Scheduler,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger_scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-workers"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_use(expected_workers(
                WorkloadParallelRemoteFlowScope::Scheduler,
                3,
            ))
            .unwrap()
            .build()
            .unwrap();
    let full_system =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-workers"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_use(expected_workers(
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
fn workload_replay_plan_rejects_missing_or_underused_parallel_workers() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-workers-mismatch"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_parallel_worker_use(expected_workers(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelWorkerSummary {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_max_workers: 2,
        },
    );

    let serial_summary =
        WorkloadParallelExecutionSummary::default().with_scheduler_partitions(2, 1);
    let serial_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(serial_summary);
    assert_eq!(
        plan.verify_result(&serial_result).unwrap_err(),
        WorkloadError::ExpectedParallelWorkerCountBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_max_workers: 2,
            actual_max_workers: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_parallel_worker_expectations() {
    let zero =
        WorkloadExpectedParallelWorkerUse::new(WorkloadParallelRemoteFlowScope::FullSystem, 0)
            .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedParallelWorkerCount {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
        },
    );

    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-workers-duplicate"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_worker_use(expected_workers(
                WorkloadParallelRemoteFlowScope::FullSystem,
                2,
            ))
            .unwrap()
            .add_expected_parallel_worker_use(expected_workers(
                WorkloadParallelRemoteFlowScope::FullSystem,
                3,
            ))
            .unwrap_err();
    assert_eq!(
        manifest,
        WorkloadError::DuplicateExpectedParallelWorkerUse {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
        },
    );
}

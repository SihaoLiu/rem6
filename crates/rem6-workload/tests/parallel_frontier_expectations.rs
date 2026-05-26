use rem6_boot::BootImage;
use rem6_kernel::{PartitionFrontier, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelFrontier, WorkloadId, WorkloadParallelExecutionSummary,
    WorkloadParallelFrontierStage, WorkloadParallelRemoteFlowScope, WorkloadParallelSchedulerScope,
    WorkloadReplayPlan, WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
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
    let manifest = rem6_workload::WorkloadManifest::builder(id("parallel-frontier"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_frontier(
    scope: WorkloadParallelRemoteFlowScope,
    stage: WorkloadParallelFrontierStage,
    partition: u32,
    minimum_now: u64,
    minimum_safe_until: u64,
) -> WorkloadExpectedParallelFrontier {
    WorkloadExpectedParallelFrontier::new(
        scope,
        stage,
        PartitionId::new(partition),
        minimum_now,
        minimum_safe_until,
    )
    .unwrap()
}

fn expected_scheduler_frontier(
    scope: WorkloadParallelSchedulerScope,
    stage: WorkloadParallelFrontierStage,
    partition: u32,
    minimum_now: u64,
    minimum_safe_until: u64,
) -> WorkloadExpectedParallelFrontier {
    WorkloadExpectedParallelFrontier::new(
        scope,
        stage,
        PartitionId::new(partition),
        minimum_now,
        minimum_safe_until,
    )
    .unwrap()
}

#[test]
fn workload_manifest_records_parallel_frontier_expectations() {
    let scheduler_initial = expected_frontier(
        WorkloadParallelRemoteFlowScope::Scheduler,
        WorkloadParallelFrontierStage::Initial,
        0,
        0,
        8,
    );
    let full_system_final = expected_frontier(
        WorkloadParallelRemoteFlowScope::FullSystem,
        WorkloadParallelFrontierStage::Final,
        4,
        21,
        29,
    );
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-parallel-frontier"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_frontier(full_system_final)
            .unwrap()
            .add_expected_parallel_frontier(scheduler_initial)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_frontiers(),
        &[scheduler_initial, full_system_final],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_frontiers(),
        manifest.expected_parallel_frontiers(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_frontiers(
            [PartitionFrontier::new(
                PartitionId::new(0),
                0,
                8,
                Some(2),
                1,
            )],
            [PartitionFrontier::new(PartitionId::new(0), 8, 16, None, 0)],
        )
        .with_data_cache_parallel_scheduler_frontiers(
            [PartitionFrontier::new(
                PartitionId::new(4),
                13,
                21,
                Some(19),
                2,
            )],
            [PartitionFrontier::new(PartitionId::new(4), 21, 29, None, 0)],
        );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_frontier_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-frontier"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let final_frontier =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-frontier"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_frontier(expected_frontier(
                WorkloadParallelRemoteFlowScope::FullSystem,
                WorkloadParallelFrontierStage::Final,
                4,
                21,
                29,
            ))
            .unwrap()
            .build()
            .unwrap();
    let stronger =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-frontier"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_frontier(expected_frontier(
                WorkloadParallelRemoteFlowScope::FullSystem,
                WorkloadParallelFrontierStage::Final,
                4,
                22,
                29,
            ))
            .unwrap()
            .build()
            .unwrap();
    let initial_frontier =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-frontier"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_frontier(expected_frontier(
                WorkloadParallelRemoteFlowScope::FullSystem,
                WorkloadParallelFrontierStage::Initial,
                4,
                21,
                29,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), final_frontier.identity());
    assert_ne!(final_frontier.identity(), stronger.identity());
    assert_ne!(final_frontier.identity(), initial_frontier.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underadvanced_parallel_frontiers() {
    let plan = replay_plan()
        .add_expected_parallel_frontier(expected_frontier(
            WorkloadParallelRemoteFlowScope::FullSystem,
            WorkloadParallelFrontierStage::Final,
            4,
            21,
            29,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelFrontierSummary {
            scope: WorkloadParallelSchedulerScope::FullSystem,
            stage: WorkloadParallelFrontierStage::Final,
            partition: 4,
            minimum_now: 21,
            minimum_safe_until: 29,
        },
    );

    let underadvanced_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_parallel_scheduler_frontiers(
            [PartitionFrontier::new(PartitionId::new(4), 13, 21, None, 0)],
            [PartitionFrontier::new(PartitionId::new(4), 20, 28, None, 0)],
        );
    let underadvanced = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underadvanced_summary);
    assert_eq!(
        plan.verify_result(&underadvanced).unwrap_err(),
        WorkloadError::ExpectedParallelFrontierBelowMinimum {
            scope: WorkloadParallelSchedulerScope::FullSystem,
            stage: WorkloadParallelFrontierStage::Final,
            partition: 4,
            minimum_now: 21,
            actual_now: Some(20),
            minimum_safe_until: 29,
            actual_safe_until: Some(28),
        },
    );

    let mixed_full_system_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_frontiers(
            [PartitionFrontier::new(PartitionId::new(4), 0, 8, None, 0)],
            [PartitionFrontier::new(PartitionId::new(4), 21, 29, None, 0)],
        )
        .with_data_cache_parallel_scheduler_frontiers(
            [PartitionFrontier::new(PartitionId::new(4), 0, 8, None, 0)],
            [PartitionFrontier::new(PartitionId::new(4), 20, 28, None, 0)],
        );
    let mixed_full_system = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(mixed_full_system_summary);
    assert_eq!(
        plan.verify_result(&mixed_full_system).unwrap_err(),
        WorkloadError::ExpectedParallelFrontierBelowMinimum {
            scope: WorkloadParallelSchedulerScope::FullSystem,
            stage: WorkloadParallelFrontierStage::Final,
            partition: 4,
            minimum_now: 21,
            actual_now: Some(20),
            minimum_safe_until: 29,
            actual_safe_until: Some(28),
        },
    );
}

#[test]
fn workload_replay_plan_checks_dma_scheduler_frontiers_directly() {
    let gpu_initial = PartitionFrontier::new(PartitionId::new(8), 10, 20, Some(14), 1);
    let gpu_final = PartitionFrontier::new(PartitionId::new(8), 18, 28, None, 0);
    let accelerator_initial = PartitionFrontier::new(PartitionId::new(9), 30, 40, Some(36), 2);
    let accelerator_final = PartitionFrontier::new(PartitionId::new(9), 45, 55, None, 0);
    let plan = replay_plan()
        .add_expected_parallel_frontier(expected_scheduler_frontier(
            WorkloadParallelSchedulerScope::GpuDmaScheduler,
            WorkloadParallelFrontierStage::Initial,
            8,
            10,
            20,
        ))
        .unwrap()
        .add_expected_parallel_frontier(expected_scheduler_frontier(
            WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
            WorkloadParallelFrontierStage::Final,
            9,
            45,
            55,
        ))
        .unwrap()
        .add_expected_parallel_frontier(expected_scheduler_frontier(
            WorkloadParallelSchedulerScope::FullSystem,
            WorkloadParallelFrontierStage::Final,
            9,
            45,
            55,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_frontiers([gpu_initial], [gpu_final])
        .with_accelerator_dma_scheduler_frontiers([accelerator_initial], [accelerator_final]);
    assert_eq!(
        summary.gpu_dma_scheduler_initial_frontiers(),
        &[gpu_initial]
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_final_frontiers(),
        &[accelerator_final],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_final_frontiers(),
        vec![gpu_final, accelerator_final],
    );
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();

    let underadvanced_summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_frontiers(
            [PartitionFrontier::new(
                PartitionId::new(8),
                9,
                19,
                Some(14),
                1,
            )],
            [gpu_final],
        )
        .with_accelerator_dma_scheduler_frontiers([accelerator_initial], [accelerator_final]);
    let underadvanced = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underadvanced_summary);
    assert_eq!(
        plan.verify_result(&underadvanced).unwrap_err(),
        WorkloadError::ExpectedParallelFrontierBelowMinimum {
            scope: WorkloadParallelSchedulerScope::GpuDmaScheduler,
            stage: WorkloadParallelFrontierStage::Initial,
            partition: 8,
            minimum_now: 10,
            actual_now: Some(9),
            minimum_safe_until: 20,
            actual_safe_until: Some(19),
        },
    );
}

#[test]
fn workload_replay_plan_rejects_inconsistent_scheduler_frontier_summary() {
    let plan = replay_plan()
        .add_expected_parallel_frontier(expected_frontier(
            WorkloadParallelRemoteFlowScope::Scheduler,
            WorkloadParallelFrontierStage::Final,
            4,
            21,
            29,
        ))
        .unwrap();

    let invalid_summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(1, 2, 1, 1)
        .with_parallel_scheduler_frontiers(
            [PartitionFrontier::new(PartitionId::new(4), 0, 8, None, 0)],
            [PartitionFrontier::new(PartitionId::new(4), 21, 29, None, 0)],
        );
    let invalid = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(invalid_summary);

    assert_eq!(
        plan.verify_result(&invalid).unwrap_err(),
        WorkloadError::InvalidParallelSchedulerSummary {
            scope: WorkloadParallelSchedulerScope::Scheduler,
            epoch_count: 1,
            empty_epoch_count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_frontiers() {
    let zero = WorkloadExpectedParallelFrontier::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        WorkloadParallelFrontierStage::Initial,
        PartitionId::new(0),
        0,
        0,
    )
    .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedParallelFrontier {
            scope: WorkloadParallelSchedulerScope::Scheduler,
            stage: WorkloadParallelFrontierStage::Initial,
            partition: 0,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_frontier(expected_frontier(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            WorkloadParallelFrontierStage::Final,
            2,
            8,
            16,
        ))
        .unwrap()
        .add_expected_parallel_frontier(expected_frontier(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            WorkloadParallelFrontierStage::Final,
            2,
            9,
            17,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelFrontier {
            scope: WorkloadParallelSchedulerScope::DataCacheScheduler,
            stage: WorkloadParallelFrontierStage::Final,
            partition: 2,
        },
    );
}

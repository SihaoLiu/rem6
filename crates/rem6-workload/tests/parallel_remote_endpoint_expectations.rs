use rem6_boot::BootImage;
use rem6_kernel::{ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelRemoteEndpoints, WorkloadId,
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

fn endpoint_expectation(
    scope: WorkloadParallelRemoteFlowScope,
    sources: &[u32],
    targets: &[u32],
) -> WorkloadExpectedParallelRemoteEndpoints {
    WorkloadExpectedParallelRemoteEndpoints::new(
        scope,
        sources.iter().copied().map(PartitionId::new),
        targets.iter().copied().map(PartitionId::new),
    )
    .unwrap()
}

#[test]
fn workload_manifest_records_parallel_remote_endpoint_expectations() {
    let scheduler = endpoint_expectation(
        WorkloadParallelRemoteFlowScope::Scheduler,
        &[1, 0, 1],
        &[3, 2],
    );
    let data_cache = endpoint_expectation(
        WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        &[4],
        &[2],
    );
    let full_system = endpoint_expectation(
        WorkloadParallelRemoteFlowScope::FullSystem,
        &[0, 1, 4],
        &[2, 3],
    );

    assert_eq!(
        scheduler.source_partitions(),
        &[PartitionId::new(0), PartitionId::new(1)],
    );
    assert_eq!(
        scheduler.target_partitions(),
        &[PartitionId::new(2), PartitionId::new(3)],
    );

    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-parallel-remote-endpoints"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_remote_endpoints(full_system.clone())
    .unwrap()
    .add_expected_parallel_remote_endpoints(data_cache.clone())
    .unwrap()
    .add_expected_parallel_remote_endpoints(scheduler.clone())
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(
        manifest.expected_parallel_remote_endpoints(),
        &[scheduler, data_cache, full_system],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_remote_endpoints(),
        manifest.expected_parallel_remote_endpoints(),
    );
}

#[test]
fn workload_manifest_identity_changes_with_parallel_remote_endpoint_expectations() {
    let base = rem6_workload::WorkloadManifest::builder(
        id("identity-parallel-remote-endpoints"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let scheduler = rem6_workload::WorkloadManifest::builder(
        id("identity-parallel-remote-endpoints"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_remote_endpoints(endpoint_expectation(
        WorkloadParallelRemoteFlowScope::Scheduler,
        &[0],
        &[2],
    ))
    .unwrap()
    .build()
    .unwrap();
    let full_system = rem6_workload::WorkloadManifest::builder(
        id("identity-parallel-remote-endpoints"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_remote_endpoints(endpoint_expectation(
        WorkloadParallelRemoteFlowScope::FullSystem,
        &[0],
        &[2],
    ))
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), full_system.identity());
}

#[test]
fn workload_replay_plan_validates_parallel_remote_endpoint_expectations() {
    let plan = WorkloadReplayPlan::from_manifest(
        &rem6_workload::WorkloadManifest::builder(
            id("validate-parallel-remote-endpoints"),
            boot_image(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_parallel_remote_endpoints(endpoint_expectation(
            WorkloadParallelRemoteFlowScope::Scheduler,
            &[0, 1],
            &[2, 3],
        ))
        .unwrap()
        .add_expected_parallel_remote_endpoints(endpoint_expectation(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            &[4],
            &[2],
        ))
        .unwrap()
        .add_expected_parallel_remote_endpoints(endpoint_expectation(
            WorkloadParallelRemoteFlowScope::FullSystem,
            &[0, 1, 4],
            &[2, 3],
        ))
        .unwrap()
        .build()
        .unwrap(),
    )
    .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(2),
            2,
            3,
            7,
        )])
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(1),
            PartitionId::new(3),
            5,
            11,
            0,
        )])
        .with_data_cache_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(4),
            PartitionId::new(2),
            1,
            13,
            19,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_validates_direct_dma_scheduler_remote_endpoint_expectations() {
    let plan = WorkloadReplayPlan::from_manifest(
        &rem6_workload::WorkloadManifest::builder(
            id("validate-dma-remote-endpoints"),
            boot_image(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_parallel_remote_endpoints(endpoint_expectation(
            WorkloadParallelRemoteFlowScope::GpuDmaScheduler,
            &[6],
            &[9],
        ))
        .unwrap()
        .add_expected_parallel_remote_endpoints(endpoint_expectation(
            WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler,
            &[7],
            &[10],
        ))
        .unwrap()
        .build()
        .unwrap(),
    )
    .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(6),
            PartitionId::new(9),
            3,
            11,
            0,
        )])
        .with_accelerator_dma_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(7),
            PartitionId::new(10),
            1,
            10,
            10,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_missing_or_mismatched_parallel_remote_endpoints() {
    let plan = WorkloadReplayPlan::from_manifest(
        &rem6_workload::WorkloadManifest::builder(
            id("reject-parallel-remote-endpoints"),
            boot_image(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_parallel_remote_endpoints(endpoint_expectation(
            WorkloadParallelRemoteFlowScope::Scheduler,
            &[0, 1],
            &[2, 3],
        ))
        .unwrap()
        .build()
        .unwrap(),
    )
    .unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing).unwrap_err(),
        WorkloadError::MissingParallelRemoteEndpointSummary {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            expected_sources: vec![0, 1],
            expected_targets: vec![2, 3],
        },
    );

    let mismatched_summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 1, 3, 7),
        ]);
    let mismatched = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(mismatched_summary);
    assert_eq!(
        plan.verify_result(&mismatched).unwrap_err(),
        WorkloadError::ExpectedParallelRemoteEndpointsMismatch {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            expected_sources: vec![0, 1],
            actual_sources: vec![0],
            expected_targets: vec![2, 3],
            actual_targets: vec![2],
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_remote_endpoints() {
    assert_eq!(
        WorkloadExpectedParallelRemoteEndpoints::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            [],
            [PartitionId::new(1)],
        )
        .unwrap_err(),
        WorkloadError::EmptyExpectedParallelRemoteEndpointSources {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );
    assert_eq!(
        WorkloadExpectedParallelRemoteEndpoints::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            [PartitionId::new(0)],
            [],
        )
        .unwrap_err(),
        WorkloadError::EmptyExpectedParallelRemoteEndpointTargets {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );
    assert_eq!(
        WorkloadExpectedParallelRemoteEndpoints::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            [PartitionId::new(0)],
            [PartitionId::new(0)],
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelRemoteEndpointOverlap {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source_partitions: vec![0],
            target_partitions: vec![0],
        },
    );
    assert_eq!(
        WorkloadExpectedParallelRemoteEndpoints::new(
            WorkloadParallelRemoteFlowScope::Scheduler,
            [PartitionId::new(0), PartitionId::new(1)],
            [PartitionId::new(1), PartitionId::new(2)],
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelRemoteEndpointOverlap {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source_partitions: vec![0, 1],
            target_partitions: vec![1, 2],
        },
    );

    let duplicate = rem6_workload::WorkloadManifest::builder(
        id("duplicate-parallel-remote-endpoints"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_remote_endpoints(endpoint_expectation(
        WorkloadParallelRemoteFlowScope::Scheduler,
        &[0],
        &[2],
    ))
    .unwrap()
    .add_expected_parallel_remote_endpoints(endpoint_expectation(
        WorkloadParallelRemoteFlowScope::Scheduler,
        &[1],
        &[3],
    ))
    .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelRemoteEndpoints {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );
}

use rem6_boot::BootImage;
use rem6_kernel::{ParallelRemoteFlowRecord, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelRemoteFlow, WorkloadId,
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

fn expected(
    scope: WorkloadParallelRemoteFlowScope,
    source: u32,
    target: u32,
    send_count: usize,
) -> WorkloadExpectedParallelRemoteFlow {
    WorkloadExpectedParallelRemoteFlow::new(
        scope,
        PartitionId::new(source),
        PartitionId::new(target),
        send_count,
    )
    .unwrap()
}

#[test]
fn workload_manifest_records_parallel_remote_flow_expectations() {
    let scheduler_flow = expected(WorkloadParallelRemoteFlowScope::Scheduler, 0, 1, 2);
    let full_system_flow = expected(WorkloadParallelRemoteFlowScope::FullSystem, 0, 1, 5);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-parallel-flows"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_flow(full_system_flow)
            .unwrap()
            .add_expected_parallel_remote_flow(scheduler_flow)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_remote_flows(),
        &[scheduler_flow, full_system_flow],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_remote_flows(),
        manifest.expected_parallel_remote_flows(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(1),
            2,
            3,
            7,
        )])
        .with_data_cache_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(1),
            3,
            11,
            17,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_remote_flow_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-flows"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-flows"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_flow(expected(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
                1,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let full_system =
        rem6_workload::WorkloadManifest::builder(id("identity-parallel-flows"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_flow(expected(
                WorkloadParallelRemoteFlowScope::FullSystem,
                0,
                1,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), full_system.identity());
}

#[test]
fn workload_manifest_rejects_duplicate_parallel_remote_flow_expectations() {
    let error =
        rem6_workload::WorkloadManifest::builder(id("duplicate-parallel-flows"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_flow(expected(
                WorkloadParallelRemoteFlowScope::FullSystem,
                0,
                1,
                2,
            ))
            .unwrap()
            .add_expected_parallel_remote_flow(expected(
                WorkloadParallelRemoteFlowScope::FullSystem,
                0,
                1,
                3,
            ))
            .unwrap_err();

    assert_eq!(
        error,
        WorkloadError::DuplicateExpectedParallelRemoteFlow {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            source: 0,
            target: 1,
        },
    );
}

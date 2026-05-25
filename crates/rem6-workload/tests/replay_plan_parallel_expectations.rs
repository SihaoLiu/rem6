use rem6_boot::BootImage;
use rem6_kernel::{ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId};
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

fn replay_plan() -> WorkloadReplayPlan {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-flow-expectations"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
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
fn workload_replay_plan_validates_expected_parallel_remote_flows() {
    let plan = replay_plan()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            2,
        ))
        .unwrap()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            0,
            1,
            3,
        ))
        .unwrap()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::FullSystem,
            0,
            1,
            5,
        ))
        .unwrap();
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

    assert_eq!(
        plan.expected_parallel_remote_flows(),
        &[
            expected(WorkloadParallelRemoteFlowScope::Scheduler, 0, 1, 2),
            expected(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 0, 1, 3,),
            expected(WorkloadParallelRemoteFlowScope::FullSystem, 0, 1, 5),
        ],
    );
    assert_eq!(
        WorkloadParallelRemoteFlowScope::Scheduler.as_str(),
        "scheduler",
    );
    assert_eq!(
        WorkloadParallelRemoteFlowScope::DataCacheScheduler.as_str(),
        "data-cache-scheduler",
    );
    assert_eq!(
        WorkloadParallelRemoteFlowScope::FullSystem.as_str(),
        "full-system",
    );
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_derives_parallel_remote_flows_from_remote_sends() {
    let plan = replay_plan()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            2,
        ))
        .unwrap()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            2,
            3,
            1,
        ))
        .unwrap()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::FullSystem,
            0,
            1,
            2,
        ))
        .unwrap()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::FullSystem,
            2,
            3,
            1,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(1),
                3,
                11,
                0,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(1),
                7,
                17,
                1,
            ),
        ])
        .with_data_cache_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(2),
            PartitionId::new(3),
            5,
            13,
            0,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_missing_or_mismatched_parallel_remote_flow() {
    let plan = replay_plan()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            2,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    let error = plan.verify_result(&missing_summary).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingParallelExecutionSummary {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            expected_send_count: 2,
        },
    );

    let mismatched_summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(1), 1, 3, 7),
        ]);
    let mismatched = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(mismatched_summary);
    let error = plan.verify_result(&mismatched).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::ExpectedParallelRemoteFlowCountMismatch {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            expected_send_count: 2,
            actual_send_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_unexpected_parallel_remote_flow() {
    let plan = replay_plan()
        .add_expected_parallel_remote_flow(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            2,
        ))
        .unwrap();

    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(1), 2, 3, 7),
            ParallelRemoteFlowRecord::new(PartitionId::new(2), PartitionId::new(3), 1, 11, 11),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::UnexpectedParallelRemoteFlow {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 2,
            target: 3,
            actual_send_count: 1,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_parallel_remote_flow_expectations() {
    let zero = WorkloadExpectedParallelRemoteFlow::new(
        WorkloadParallelRemoteFlowScope::FullSystem,
        PartitionId::new(0),
        PartitionId::new(1),
        0,
    )
    .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedParallelRemoteFlowCount {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            source: 0,
            target: 1,
        },
    );

    let duplicate = replay_plan()
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
        duplicate,
        WorkloadError::DuplicateExpectedParallelRemoteFlow {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            source: 0,
            target: 1,
        },
    );
}

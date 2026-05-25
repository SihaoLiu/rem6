use rem6_boot::BootImage;
use rem6_kernel::{ParallelRemoteFlowRecord, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelRemoteFlowTiming, WorkloadId,
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
        rem6_workload::WorkloadManifest::builder(id("parallel-flow-timing"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_timing(
    scope: WorkloadParallelRemoteFlowScope,
    source: u32,
    target: u32,
    send_count: usize,
    first_tick: u64,
    last_tick: u64,
) -> WorkloadExpectedParallelRemoteFlowTiming {
    WorkloadExpectedParallelRemoteFlowTiming::new(
        scope,
        PartitionId::new(source),
        PartitionId::new(target),
        send_count,
        first_tick,
        last_tick,
    )
    .unwrap()
}

#[test]
fn workload_manifest_records_parallel_remote_flow_timing_expectations() {
    let scheduler_timing =
        expected_timing(WorkloadParallelRemoteFlowScope::Scheduler, 0, 1, 2, 3, 7);
    let data_cache_timing = expected_timing(
        WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        0,
        1,
        3,
        11,
        17,
    );
    let full_system_timing =
        expected_timing(WorkloadParallelRemoteFlowScope::FullSystem, 0, 1, 5, 3, 17);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-flow-timing"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_flow_timing(full_system_timing)
            .unwrap()
            .add_expected_parallel_remote_flow_timing(data_cache_timing)
            .unwrap()
            .add_expected_parallel_remote_flow_timing(scheduler_timing)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_parallel_remote_flow_timings(),
        &[scheduler_timing, data_cache_timing, full_system_timing],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_remote_flow_timings(),
        manifest.expected_parallel_remote_flow_timings(),
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
fn workload_manifest_identity_changes_with_parallel_remote_flow_timing() {
    let base = rem6_workload::WorkloadManifest::builder(id("identity-flow-timing"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let early = rem6_workload::WorkloadManifest::builder(id("identity-flow-timing"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_parallel_remote_flow_timing(expected_timing(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            2,
            3,
            7,
        ))
        .unwrap()
        .build()
        .unwrap();
    let late = rem6_workload::WorkloadManifest::builder(id("identity-flow-timing"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_parallel_remote_flow_timing(expected_timing(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            2,
            3,
            11,
        ))
        .unwrap()
        .build()
        .unwrap();

    assert_ne!(base.identity(), early.identity());
    assert_ne!(early.identity(), late.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_mismatched_parallel_remote_flow_timing() {
    let plan = replay_plan()
        .add_expected_parallel_remote_flow_timing(expected_timing(
            WorkloadParallelRemoteFlowScope::FullSystem,
            0,
            1,
            5,
            3,
            17,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelRemoteFlowTimingSummary {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            source: 0,
            target: 1,
            expected_send_count: 5,
            expected_first_tick: 3,
            expected_last_tick: 17,
        },
    );

    let drifted_summary = WorkloadParallelExecutionSummary::default()
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
            21,
        )]);
    let drifted = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(drifted_summary);
    assert_eq!(
        plan.verify_result(&drifted).unwrap_err(),
        WorkloadError::ExpectedParallelRemoteFlowTimingMismatch {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            source: 0,
            target: 1,
            expected_send_count: 5,
            actual_send_count: 5,
            expected_first_tick: 3,
            actual_first_tick: Some(3),
            expected_last_tick: 17,
            actual_last_tick: Some(21),
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_remote_flow_timing() {
    let invalid_window = WorkloadExpectedParallelRemoteFlowTiming::new(
        WorkloadParallelRemoteFlowScope::Scheduler,
        PartitionId::new(0),
        PartitionId::new(1),
        2,
        11,
        7,
    )
    .unwrap_err();
    assert_eq!(
        invalid_window,
        WorkloadError::InvalidExpectedParallelRemoteFlowTimingWindow {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            first_tick: 11,
            last_tick: 7,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_remote_flow_timing(expected_timing(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            2,
            3,
            7,
        ))
        .unwrap()
        .add_expected_parallel_remote_flow_timing(expected_timing(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            2,
            3,
            11,
        ))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelRemoteFlowTiming {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
        },
    );
}

use rem6_boot::BootImage;
use rem6_kernel::{ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelRemoteDelayCeiling,
    WorkloadExpectedParallelRemoteDelayFloor, WorkloadId, WorkloadParallelExecutionSummary,
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
        rem6_workload::WorkloadManifest::builder(id("parallel-remote-delay-ceiling"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_ceiling(
    scope: WorkloadParallelRemoteFlowScope,
    maximum_delay: u64,
) -> WorkloadExpectedParallelRemoteDelayCeiling {
    WorkloadExpectedParallelRemoteDelayCeiling::new(scope, maximum_delay)
}

fn expected_floor(
    scope: WorkloadParallelRemoteFlowScope,
    minimum_delay: u64,
) -> WorkloadExpectedParallelRemoteDelayFloor {
    WorkloadExpectedParallelRemoteDelayFloor::new(scope, minimum_delay).unwrap()
}

#[test]
fn workload_manifest_records_parallel_remote_delay_ceiling_expectations() {
    let scheduler = expected_ceiling(WorkloadParallelRemoteFlowScope::Scheduler, 8);
    let data_cache = expected_ceiling(WorkloadParallelRemoteFlowScope::DataCacheScheduler, 9);
    let full_system = expected_ceiling(WorkloadParallelRemoteFlowScope::FullSystem, 10);

    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-remote-delay-ceiling"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_delay_ceiling(full_system)
            .unwrap()
            .add_expected_parallel_remote_delay_ceiling(data_cache)
            .unwrap()
            .add_expected_parallel_remote_delay_ceiling(scheduler)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        scheduler.scope(),
        WorkloadParallelRemoteFlowScope::Scheduler
    );
    assert_eq!(scheduler.maximum_delay(), 8);
    assert_eq!(
        manifest.expected_parallel_remote_delay_ceilings(),
        &[scheduler, data_cache, full_system],
    );

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_remote_delay_ceilings(),
        manifest.expected_parallel_remote_delay_ceilings(),
    );
}

#[test]
fn workload_manifest_identity_changes_with_parallel_remote_delay_ceiling_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-remote-delay-ceiling"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let scheduler =
        rem6_workload::WorkloadManifest::builder(id("identity-remote-delay-ceiling"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
                WorkloadParallelRemoteFlowScope::Scheduler,
                8,
            ))
            .unwrap()
            .build()
            .unwrap();
    let wider_ceiling =
        rem6_workload::WorkloadManifest::builder(id("identity-remote-delay-ceiling"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
                WorkloadParallelRemoteFlowScope::Scheduler,
                12,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), wider_ceiling.identity());
}

#[test]
fn workload_replay_plan_validates_parallel_remote_delay_ceilings() {
    let plan = replay_plan()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::Scheduler,
            8,
        ))
        .unwrap()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            9,
        ))
        .unwrap()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::FullSystem,
            9,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(1),
                3,
                10,
                0,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(1),
                PartitionId::new(2),
                5,
                13,
                1,
            ),
        ])
        .with_data_cache_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::with_delay_bounds(
                PartitionId::new(3),
                PartitionId::new(4),
                2,
                17,
                19,
                5,
                9,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_missing_or_unbounded_parallel_remote_delay_ceiling_evidence() {
    let plan = replay_plan()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::Scheduler,
            8,
        ))
        .unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing).unwrap_err(),
        WorkloadError::MissingParallelRemoteDelayCeilingSummary {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            maximum_delay: 8,
        },
    );

    let empty_summary = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(WorkloadParallelExecutionSummary::default());
    assert_eq!(
        plan.verify_result(&empty_summary).unwrap_err(),
        WorkloadError::MissingParallelRemoteDelayCeilingEvidence {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            maximum_delay: 8,
        },
    );

    let unbounded_summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(1), 2, 11, 17),
        ]);
    let unbounded = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(unbounded_summary);
    assert_eq!(
        plan.verify_result(&unbounded).unwrap_err(),
        WorkloadError::MissingParallelRemoteFlowMaximumDelayEvidence {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            maximum_delay: 8,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_remote_delay_above_ceiling() {
    let plan = replay_plan()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::FullSystem,
            8,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(1),
                3,
                12,
                0,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::ExpectedParallelRemoteDelayAboveCeiling {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            source: 0,
            target: 1,
            maximum_delay: 8,
            actual_maximum_delay: 9,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_inverted_actual_parallel_remote_delay_ceiling_send_evidence() {
    let plan = replay_plan()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::Scheduler,
            10,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(2),
                PartitionId::new(3),
                12,
                4,
                0,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficSendTiming {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 2,
            target: 3,
            source_tick: 12,
            delivery_tick: 4,
            order: 0,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_remote_delay_ceilings() {
    assert_eq!(
        replay_plan()
            .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
            ))
            .unwrap_err(),
        WorkloadError::ZeroExpectedParallelRemoteDelayCeiling {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );

    assert_eq!(
        rem6_workload::WorkloadManifest::builder(id("invalid-remote-delay-ceiling"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
            ))
            .unwrap_err(),
        WorkloadError::ZeroExpectedParallelRemoteDelayCeiling {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );
    assert_eq!(
        replay_plan()
            .add_expected_parallel_remote_delay_floor(expected_floor(
                WorkloadParallelRemoteFlowScope::Scheduler,
                8,
            ))
            .unwrap()
            .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
                WorkloadParallelRemoteFlowScope::Scheduler,
                4,
            ))
            .unwrap_err(),
        WorkloadError::InvalidExpectedParallelRemoteDelayWindow {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_delay: 8,
            maximum_delay: 4,
        },
    );

    assert_eq!(
        rem6_workload::WorkloadManifest::builder(
            id("invalid-remote-delay-ceiling-window"),
            boot_image(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_parallel_remote_delay_floor(expected_floor(
            WorkloadParallelRemoteFlowScope::Scheduler,
            8,
        ))
        .unwrap()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::Scheduler,
            4,
        ))
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelRemoteDelayWindow {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            minimum_delay: 8,
            maximum_delay: 4,
        },
    );

    let duplicate = replay_plan()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::Scheduler,
            8,
        ))
        .unwrap()
        .add_expected_parallel_remote_delay_ceiling(expected_ceiling(
            WorkloadParallelRemoteFlowScope::Scheduler,
            12,
        ))
        .unwrap_err();

    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelRemoteDelayCeiling {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );
}

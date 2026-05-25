use rem6_boot::BootImage;
use rem6_kernel::{ParallelRemoteSendRecord, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelRemoteSend, WorkloadId,
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
        rem6_workload::WorkloadManifest::builder(id("parallel-remote-send"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_send(
    scope: WorkloadParallelRemoteFlowScope,
    source: u32,
    target: u32,
    source_tick: u64,
    delivery_tick: u64,
    order: u64,
) -> WorkloadExpectedParallelRemoteSend {
    WorkloadExpectedParallelRemoteSend::new(
        scope,
        PartitionId::new(source),
        PartitionId::new(target),
        source_tick,
        delivery_tick,
        order,
    )
}

#[test]
fn workload_manifest_records_parallel_remote_send_expectations() {
    let scheduler_send = expected_send(WorkloadParallelRemoteFlowScope::Scheduler, 0, 1, 3, 11, 0);
    let data_cache_send = expected_send(
        WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        2,
        3,
        5,
        13,
        1,
    );
    let full_system_send =
        expected_send(WorkloadParallelRemoteFlowScope::FullSystem, 0, 1, 3, 11, 0);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-remote-send"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_send(full_system_send)
            .unwrap()
            .add_expected_parallel_remote_send(data_cache_send)
            .unwrap()
            .add_expected_parallel_remote_send(scheduler_send)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(scheduler_send.source(), PartitionId::new(0));
    assert_eq!(scheduler_send.target(), PartitionId::new(1));
    assert_eq!(scheduler_send.source_tick(), 3);
    assert_eq!(scheduler_send.delivery_tick(), 11);
    assert_eq!(scheduler_send.delay(), 8);
    assert_eq!(scheduler_send.order(), 0);
    assert_eq!(
        manifest.expected_parallel_remote_sends(),
        &[scheduler_send, data_cache_send, full_system_send],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_remote_sends(),
        manifest.expected_parallel_remote_sends(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(0),
            PartitionId::new(1),
            3,
            11,
            0,
        )])
        .with_data_cache_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(2),
            PartitionId::new(3),
            5,
            13,
            1,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_remote_send_expectations() {
    let base = rem6_workload::WorkloadManifest::builder(id("identity-remote-send"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_parallel_remote_send(expected_send(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            3,
            11,
            0,
        ))
        .unwrap()
        .build()
        .unwrap();
    let later_delivery =
        rem6_workload::WorkloadManifest::builder(id("identity-remote-send"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_send(expected_send(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
                1,
                3,
                12,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let later_order =
        rem6_workload::WorkloadManifest::builder(id("identity-remote-send"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_remote_send(expected_send(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
                1,
                3,
                11,
                1,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), later_delivery.identity());
    assert_ne!(base.identity(), later_order.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_parallel_remote_send() {
    let plan = replay_plan()
        .add_expected_parallel_remote_send(expected_send(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            3,
            11,
            0,
        ))
        .unwrap();

    let no_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&no_summary).unwrap_err(),
        WorkloadError::MissingParallelRemoteSendSummary {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            source_tick: 3,
            delivery_tick: 11,
            order: 0,
        },
    );

    let wrong_send_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(0),
            PartitionId::new(1),
            3,
            12,
            0,
        )]);
    let wrong_send = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(wrong_send_summary);
    assert_eq!(
        plan.verify_result(&wrong_send).unwrap_err(),
        WorkloadError::ExpectedParallelRemoteSendMissing {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            source_tick: 3,
            delivery_tick: 11,
            order: 0,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_parallel_remote_send_expectations() {
    let duplicate = replay_plan()
        .add_expected_parallel_remote_send(expected_send(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            3,
            11,
            0,
        ))
        .unwrap()
        .add_expected_parallel_remote_send(expected_send(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            1,
            3,
            11,
            0,
        ))
        .unwrap_err();

    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelRemoteSend {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            source_tick: 3,
            delivery_tick: 11,
            order: 0,
        },
    );
}

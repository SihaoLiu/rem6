use rem6_boot::BootImage;
use rem6_kernel::{
    LivelockTransitionKind, ParallelProgressTransitionRecord, PartitionId, WaitForNode,
};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelProgressTransition, WorkloadId,
    WorkloadParallelExecutionSummary, WorkloadParallelProgressTransitionExpectationError,
    WorkloadParallelProgressTransitionExpectationFailure, WorkloadParallelRemoteFlowScope,
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
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-progress-transition"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn subject(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}

fn expected_transition(
    scope: WorkloadParallelRemoteFlowScope,
    partition: u32,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: u64,
    order: u64,
) -> WorkloadExpectedParallelProgressTransition {
    WorkloadExpectedParallelProgressTransition::new(
        scope,
        PartitionId::new(partition),
        subject,
        kind,
        tick,
        order,
    )
}

fn actual_transition(
    partition: u32,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: u64,
    order: u64,
) -> ParallelProgressTransitionRecord {
    ParallelProgressTransitionRecord::new(PartitionId::new(partition), subject, kind, tick, order)
}

fn expectation_error(
    failure: WorkloadParallelProgressTransitionExpectationFailure,
    scope: WorkloadParallelRemoteFlowScope,
    partition: u32,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: u64,
    order: u64,
) -> WorkloadError {
    WorkloadError::ParallelProgressTransitionExpectation(
        WorkloadParallelProgressTransitionExpectationError::new(
            failure,
            scope,
            PartitionId::new(partition),
            subject,
            kind,
            tick,
            order,
        ),
    )
}

#[test]
fn workload_manifest_records_parallel_progress_transition_expectations() {
    let scheduler_subject = subject("cpu-scheduler");
    let data_cache_subject = subject("data-cache-scheduler");
    let scheduler_transition = expected_transition(
        WorkloadParallelRemoteFlowScope::Scheduler,
        0,
        scheduler_subject.clone(),
        LivelockTransitionKind::SchedulerEpoch,
        3,
        0,
    );
    let data_cache_transition = expected_transition(
        WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        2,
        data_cache_subject.clone(),
        LivelockTransitionKind::QueueRotation,
        5,
        1,
    );
    let full_system_transition = expected_transition(
        WorkloadParallelRemoteFlowScope::FullSystem,
        0,
        scheduler_subject.clone(),
        LivelockTransitionKind::SchedulerEpoch,
        3,
        0,
    );
    let full_system_data_cache_transition = expected_transition(
        WorkloadParallelRemoteFlowScope::FullSystem,
        2,
        data_cache_subject.clone(),
        LivelockTransitionKind::QueueRotation,
        5,
        1,
    );
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-progress-transition"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_progress_transition(full_system_data_cache_transition.clone())
            .unwrap()
            .add_expected_parallel_progress_transition(full_system_transition.clone())
            .unwrap()
            .add_expected_parallel_progress_transition(data_cache_transition.clone())
            .unwrap()
            .add_expected_parallel_progress_transition(scheduler_transition.clone())
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(scheduler_transition.partition(), PartitionId::new(0));
    assert_eq!(scheduler_transition.subject(), &scheduler_subject);
    assert_eq!(
        scheduler_transition.kind(),
        LivelockTransitionKind::SchedulerEpoch,
    );
    assert_eq!(scheduler_transition.tick(), 3);
    assert_eq!(scheduler_transition.order(), 0);
    assert_eq!(
        manifest.expected_parallel_progress_transitions(),
        &[
            scheduler_transition.clone(),
            data_cache_transition.clone(),
            full_system_transition.clone(),
            full_system_data_cache_transition.clone(),
        ],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_progress_transitions(),
        manifest.expected_parallel_progress_transitions(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_progress_transitions([actual_transition(
            0,
            scheduler_subject,
            LivelockTransitionKind::SchedulerEpoch,
            3,
            0,
        )])
        .with_data_cache_parallel_scheduler_progress_transitions([actual_transition(
            2,
            data_cache_subject,
            LivelockTransitionKind::QueueRotation,
            5,
            1,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_parallel_progress_transition_expectations() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-progress-transition"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_progress_transition(expected_transition(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
                subject("cpu-scheduler"),
                LivelockTransitionKind::SchedulerEpoch,
                3,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let later_order =
        rem6_workload::WorkloadManifest::builder(id("identity-progress-transition"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_progress_transition(expected_transition(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
                subject("cpu-scheduler"),
                LivelockTransitionKind::SchedulerEpoch,
                3,
                1,
            ))
            .unwrap()
            .build()
            .unwrap();
    let different_kind =
        rem6_workload::WorkloadManifest::builder(id("identity-progress-transition"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_progress_transition(expected_transition(
                WorkloadParallelRemoteFlowScope::Scheduler,
                0,
                subject("cpu-scheduler"),
                LivelockTransitionKind::ProtocolRetry,
                3,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), later_order.identity());
    assert_ne!(base.identity(), different_kind.identity());
}

#[test]
fn workload_replay_plan_uses_explicit_full_system_progress_transitions() {
    let global_subject = subject("global-scheduler");
    let scoped_transition = actual_transition(
        0,
        subject("cpu-scheduler"),
        LivelockTransitionKind::SchedulerEpoch,
        3,
        0,
    );
    let full_system_transition = actual_transition(
        6,
        global_subject.clone(),
        LivelockTransitionKind::QueueRotation,
        21,
        4,
    );
    let plan = replay_plan()
        .add_expected_parallel_progress_transition(expected_transition(
            WorkloadParallelRemoteFlowScope::FullSystem,
            6,
            global_subject,
            LivelockTransitionKind::QueueRotation,
            21,
            4,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_progress_transitions([scoped_transition.clone()])
        .with_full_system_progress_transitions([full_system_transition.clone()]);

    assert_eq!(
        summary.parallel_scheduler_progress_transitions(),
        &[scoped_transition],
    );
    assert_eq!(
        summary.full_system_progress_transitions(),
        vec![full_system_transition],
    );
    assert_eq!(summary.full_system_progress_transition_count(), 1);
    assert!(summary.has_full_system_progress_transitions());

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_checks_dma_scheduler_progress_transitions_directly() {
    let gpu_subject = subject("gpu-dma-progress");
    let accelerator_subject = subject("accelerator-dma-progress");
    let plan = replay_plan()
        .add_expected_parallel_progress_transition(expected_transition(
            WorkloadParallelRemoteFlowScope::GpuDmaScheduler,
            8,
            gpu_subject.clone(),
            LivelockTransitionKind::QueueRotation,
            13,
            0,
        ))
        .unwrap()
        .add_expected_parallel_progress_transition(expected_transition(
            WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler,
            11,
            accelerator_subject.clone(),
            LivelockTransitionKind::ProtocolRetry,
            17,
            1,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_progress_transitions([actual_transition(
            8,
            gpu_subject,
            LivelockTransitionKind::QueueRotation,
            13,
            0,
        )])
        .with_accelerator_dma_scheduler_progress_transitions([actual_transition(
            11,
            accelerator_subject,
            LivelockTransitionKind::ProtocolRetry,
            17,
            1,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_derives_full_system_progress_transitions_from_dma_schedulers() {
    let gpu_subject = subject("gpu-dma-global-progress");
    let accelerator_subject = subject("accelerator-dma-global-progress");
    let plan = replay_plan()
        .add_expected_parallel_progress_transition(expected_transition(
            WorkloadParallelRemoteFlowScope::FullSystem,
            8,
            gpu_subject.clone(),
            LivelockTransitionKind::QueueRotation,
            13,
            0,
        ))
        .unwrap()
        .add_expected_parallel_progress_transition(expected_transition(
            WorkloadParallelRemoteFlowScope::FullSystem,
            11,
            accelerator_subject.clone(),
            LivelockTransitionKind::ProtocolRetry,
            17,
            1,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_gpu_dma_scheduler_progress_transitions([actual_transition(
            8,
            gpu_subject,
            LivelockTransitionKind::QueueRotation,
            13,
            0,
        )])
        .with_accelerator_dma_scheduler_progress_transitions([actual_transition(
            11,
            accelerator_subject,
            LivelockTransitionKind::ProtocolRetry,
            17,
            1,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_missing_parallel_progress_transition() {
    let expected = expected_transition(
        WorkloadParallelRemoteFlowScope::Scheduler,
        0,
        subject("cpu-scheduler"),
        LivelockTransitionKind::SchedulerEpoch,
        3,
        0,
    );
    let plan = replay_plan()
        .add_expected_parallel_progress_transition(expected.clone())
        .unwrap();

    let no_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&no_summary).unwrap_err(),
        expectation_error(
            WorkloadParallelProgressTransitionExpectationFailure::MissingSummary,
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            subject("cpu-scheduler"),
            LivelockTransitionKind::SchedulerEpoch,
            3,
            0,
        ),
    );

    let wrong_transition_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_progress_transitions([actual_transition(
            0,
            subject("cpu-scheduler"),
            LivelockTransitionKind::SchedulerEpoch,
            3,
            1,
        )]);
    let wrong_transition_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(wrong_transition_summary);
    assert_eq!(
        plan.verify_result(&wrong_transition_result).unwrap_err(),
        expectation_error(
            WorkloadParallelProgressTransitionExpectationFailure::MissingRecord,
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            subject("cpu-scheduler"),
            LivelockTransitionKind::SchedulerEpoch,
            3,
            0,
        ),
    );

    let duplicate_err = replay_plan()
        .add_expected_parallel_progress_transition(expected.clone())
        .unwrap()
        .add_expected_parallel_progress_transition(expected)
        .unwrap_err();
    assert_eq!(
        duplicate_err,
        expectation_error(
            WorkloadParallelProgressTransitionExpectationFailure::Duplicate,
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            subject("cpu-scheduler"),
            LivelockTransitionKind::SchedulerEpoch,
            3,
            0,
        ),
    );
}

#[test]
fn workload_replay_plan_rejects_unexpected_parallel_progress_transition() {
    let plan = replay_plan()
        .add_expected_parallel_progress_transition(expected_transition(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            subject("cpu-scheduler"),
            LivelockTransitionKind::SchedulerEpoch,
            3,
            0,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_progress_transitions([
            actual_transition(
                0,
                subject("cpu-scheduler"),
                LivelockTransitionKind::SchedulerEpoch,
                3,
                0,
            ),
            actual_transition(
                1,
                subject("cache-retry"),
                LivelockTransitionKind::ProtocolRetry,
                4,
                0,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        expectation_error(
            WorkloadParallelProgressTransitionExpectationFailure::UnexpectedRecord,
            WorkloadParallelRemoteFlowScope::Scheduler,
            1,
            subject("cache-retry"),
            LivelockTransitionKind::ProtocolRetry,
            4,
            0,
        ),
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_actual_parallel_progress_transition() {
    let plan = replay_plan()
        .add_expected_parallel_progress_transition(expected_transition(
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            subject("cpu-scheduler"),
            LivelockTransitionKind::SchedulerEpoch,
            3,
            0,
        ))
        .unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_progress_transitions([
            actual_transition(
                0,
                subject("cpu-scheduler"),
                LivelockTransitionKind::SchedulerEpoch,
                3,
                0,
            ),
            actual_transition(
                0,
                subject("cpu-scheduler"),
                LivelockTransitionKind::SchedulerEpoch,
                3,
                0,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        expectation_error(
            WorkloadParallelProgressTransitionExpectationFailure::UnexpectedRecord,
            WorkloadParallelRemoteFlowScope::Scheduler,
            0,
            subject("cpu-scheduler"),
            LivelockTransitionKind::SchedulerEpoch,
            3,
            0,
        ),
    );
}

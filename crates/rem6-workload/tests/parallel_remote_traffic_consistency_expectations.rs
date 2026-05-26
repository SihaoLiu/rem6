use rem6_boot::BootImage;
use rem6_kernel::{ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedParallelRemoteTrafficConsistency, WorkloadId,
    WorkloadParallelExecutionSummary, WorkloadParallelRemoteFlowScope,
    WorkloadParallelRemoteTrafficConsistencyMismatch, WorkloadReplayPlan, WorkloadResource,
    WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
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
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("parallel-remote-traffic-consistency"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected(
    scope: WorkloadParallelRemoteFlowScope,
) -> WorkloadExpectedParallelRemoteTrafficConsistency {
    WorkloadExpectedParallelRemoteTrafficConsistency::new(scope)
}

fn traffic_mismatch(mismatch: WorkloadParallelRemoteTrafficConsistencyMismatch) -> WorkloadError {
    WorkloadError::ParallelRemoteTrafficConsistencyMismatch(Box::new(mismatch))
}

#[test]
fn workload_manifest_records_parallel_remote_traffic_consistency_expectations() {
    let scheduler = expected(WorkloadParallelRemoteFlowScope::Scheduler);
    let data_cache = expected(WorkloadParallelRemoteFlowScope::DataCacheScheduler);
    let full_system = expected(WorkloadParallelRemoteFlowScope::FullSystem);
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-remote-traffic-consistency"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_remote_traffic_consistency(full_system)
    .unwrap()
    .add_expected_parallel_remote_traffic_consistency(data_cache)
    .unwrap()
    .add_expected_parallel_remote_traffic_consistency(scheduler)
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(
        scheduler.scope(),
        WorkloadParallelRemoteFlowScope::Scheduler
    );
    assert_eq!(
        manifest.expected_parallel_remote_traffic_consistency(),
        &[scheduler, data_cache, full_system],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_remote_traffic_consistency(),
        manifest.expected_parallel_remote_traffic_consistency(),
    );
}

#[test]
fn workload_manifest_identity_changes_with_parallel_remote_traffic_consistency() {
    let base = rem6_workload::WorkloadManifest::builder(
        id("identity-remote-traffic-consistency"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let scheduler = rem6_workload::WorkloadManifest::builder(
        id("identity-remote-traffic-consistency"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_remote_traffic_consistency(expected(
        WorkloadParallelRemoteFlowScope::Scheduler,
    ))
    .unwrap()
    .build()
    .unwrap();
    let full_system = rem6_workload::WorkloadManifest::builder(
        id("identity-remote-traffic-consistency"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_remote_traffic_consistency(expected(
        WorkloadParallelRemoteFlowScope::FullSystem,
    ))
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(base.identity(), scheduler.identity());
    assert_ne!(scheduler.identity(), full_system.identity());
}

#[test]
fn workload_replay_plan_validates_parallel_remote_traffic_consistency() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        ))
        .unwrap()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::FullSystem,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::with_delay_bounds(
            PartitionId::new(0),
            PartitionId::new(1),
            2,
            11,
            17,
            8,
            10,
        )])
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
        .with_data_cache_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(2),
            PartitionId::new(3),
            1,
            13,
            13,
        )])
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
fn workload_replay_plan_rejects_missing_parallel_remote_traffic_summary() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let missing = WorkloadResult::new(plan.manifest_identity(), 32);

    assert_eq!(
        plan.verify_result(&missing).unwrap_err(),
        WorkloadError::MissingParallelRemoteTrafficConsistencySummary {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_parallel_remote_traffic_without_exact_sends() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(1), 1, 11, 11),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        traffic_mismatch(WorkloadParallelRemoteTrafficConsistencyMismatch {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            flow_send_count: 1,
            send_record_count: 0,
            flow_first_tick: 11,
            send_first_tick: None,
            flow_last_tick: 11,
            send_last_tick: None,
            flow_minimum_delay: None,
            send_minimum_delay: None,
            flow_maximum_delay: None,
            send_maximum_delay: None,
        }),
    );
}

#[test]
fn workload_replay_plan_accepts_parallel_remote_traffic_from_exact_sends_without_aggregate() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(1),
                3,
                11,
                0,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_local_parallel_remote_send_evidence() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(2),
                PartitionId::new(2),
                5,
                13,
                0,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficSendEndpoints {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 2,
            target: 2,
            source_tick: 5,
            delivery_tick: 13,
            order: 0,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_inverted_parallel_remote_send_evidence_timing() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(2),
                PartitionId::new(3),
                13,
                5,
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
            source_tick: 13,
            delivery_tick: 5,
            order: 0,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_local_parallel_remote_flow_evidence() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(2), PartitionId::new(2), 1, 5, 13),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficFlowEndpoints {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 2,
            target: 2,
            send_count: 1,
            first_tick: 5,
            last_tick: 13,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_inverted_parallel_remote_flow_evidence_timing() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(2), PartitionId::new(3), 1, 13, 5),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficFlowTiming {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 2,
            target: 3,
            send_count: 1,
            first_tick: 13,
            last_tick: 5,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_full_system_hidden_inverted_remote_flow_evidence() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::FullSystem,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(1),
            1,
            3,
            3,
        )])
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(0),
            PartitionId::new(1),
            1,
            3,
            0,
        )])
        .with_data_cache_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(1),
            1,
            17,
            11,
        )])
        .with_data_cache_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(0),
            PartitionId::new(1),
            5,
            11,
            0,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficFlowTiming {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            source: 0,
            target: 1,
            send_count: 1,
            first_tick: 17,
            last_tick: 11,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_inverted_parallel_remote_flow_evidence_delay_bounds() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::with_delay_bounds(
                PartitionId::new(2),
                PartitionId::new(3),
                1,
                13,
                13,
                9,
                3,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelRemoteTrafficFlowDelayBounds {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 2,
            target: 3,
            minimum_delay: 9,
            maximum_delay: 3,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_parallel_remote_traffic_missing_aggregate_route() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(1),
            1,
            11,
            11,
        )])
        .with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(1),
                3,
                11,
                0,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(2),
                PartitionId::new(3),
                5,
                13,
                0,
            ),
        ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::MissingParallelRemoteTrafficAggregateFlow {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 2,
            target: 3,
            send_record_count: 1,
            send_first_tick: 13,
            send_last_tick: 13,
            send_minimum_delay: 8,
            send_maximum_delay: 8,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_parallel_remote_traffic_count_mismatch() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(1),
            2,
            11,
            17,
        )])
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(0),
            PartitionId::new(1),
            3,
            11,
            0,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        traffic_mismatch(WorkloadParallelRemoteTrafficConsistencyMismatch {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
            source: 0,
            target: 1,
            flow_send_count: 2,
            send_record_count: 1,
            flow_first_tick: 11,
            send_first_tick: Some(11),
            flow_last_tick: 17,
            send_last_tick: Some(11),
            flow_minimum_delay: None,
            send_minimum_delay: Some(8),
            flow_maximum_delay: None,
            send_maximum_delay: Some(8),
        }),
    );
}

#[test]
fn workload_replay_plan_rejects_parallel_remote_traffic_timing_mismatch() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::DataCacheScheduler,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(2),
            PartitionId::new(3),
            1,
            14,
            14,
        )])
        .with_data_cache_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(2),
            PartitionId::new(3),
            5,
            13,
            0,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        traffic_mismatch(WorkloadParallelRemoteTrafficConsistencyMismatch {
            scope: WorkloadParallelRemoteFlowScope::DataCacheScheduler,
            source: 2,
            target: 3,
            flow_send_count: 1,
            send_record_count: 1,
            flow_first_tick: 14,
            send_first_tick: Some(13),
            flow_last_tick: 14,
            send_last_tick: Some(13),
            flow_minimum_delay: None,
            send_minimum_delay: Some(8),
            flow_maximum_delay: None,
            send_maximum_delay: Some(8),
        }),
    );
}

#[test]
fn workload_replay_plan_rejects_parallel_remote_traffic_delay_bounds_mismatch() {
    let plan = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::FullSystem,
        ))
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::with_delay_bounds(
            PartitionId::new(0),
            PartitionId::new(1),
            1,
            11,
            11,
            9,
            9,
        )])
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(0),
            PartitionId::new(1),
            3,
            11,
            0,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        traffic_mismatch(WorkloadParallelRemoteTrafficConsistencyMismatch {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            source: 0,
            target: 1,
            flow_send_count: 1,
            send_record_count: 1,
            flow_first_tick: 11,
            send_first_tick: Some(11),
            flow_last_tick: 11,
            send_last_tick: Some(11),
            flow_minimum_delay: Some(9),
            send_minimum_delay: Some(8),
            flow_maximum_delay: Some(9),
            send_maximum_delay: Some(8),
        }),
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_parallel_remote_traffic_consistency() {
    let duplicate = replay_plan()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap()
        .add_expected_parallel_remote_traffic_consistency(expected(
            WorkloadParallelRemoteFlowScope::Scheduler,
        ))
        .unwrap_err();

    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelRemoteTrafficConsistency {
            scope: WorkloadParallelRemoteFlowScope::Scheduler,
        },
    );
}

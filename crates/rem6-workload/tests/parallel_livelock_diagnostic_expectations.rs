use rem6_boot::BootImage;
use rem6_kernel::{LivelockDiagnostic, LivelockTransitionKind, ProgressMonitor, WaitForNode};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedCleanParallelDiagnostics, WorkloadId,
    WorkloadParallelDiagnosticScope, WorkloadParallelExecutionSummary, WorkloadReplayPlan,
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

fn component(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}

fn livelock_diagnostic(
    subject: WaitForNode,
    threshold: u64,
    transitions: impl IntoIterator<Item = (LivelockTransitionKind, u64)>,
) -> LivelockDiagnostic {
    let mut monitor = ProgressMonitor::with_transition_threshold(threshold).unwrap();
    for (kind, tick) in transitions {
        monitor
            .record_transition(subject.clone(), kind, tick)
            .unwrap();
    }
    monitor.diagnostic(&subject).unwrap()
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

fn expected_clean(
    scope: WorkloadParallelDiagnosticScope,
) -> WorkloadExpectedCleanParallelDiagnostics {
    WorkloadExpectedCleanParallelDiagnostics::new(scope)
}

fn expected_clean_with_livelock_threshold(
    scope: WorkloadParallelDiagnosticScope,
    threshold: u64,
) -> WorkloadExpectedCleanParallelDiagnostics {
    WorkloadExpectedCleanParallelDiagnostics::new(scope)
        .with_livelock_transition_threshold(threshold)
        .unwrap()
}

fn replay_plan() -> WorkloadReplayPlan {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("parallel-livelock-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

#[test]
fn workload_result_records_parallel_livelock_diagnostics() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_livelock_diagnostics(5, 2)
        .with_data_cache_parallel_scheduler_livelock_diagnostics(7, 3);

    assert_eq!(summary.parallel_scheduler_progress_transition_count(), 5);
    assert_eq!(summary.parallel_scheduler_livelock_diagnostic_count(), 2);
    assert!(summary.has_parallel_scheduler_livelock_diagnostics());
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_count(),
        7,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_livelock_diagnostic_count(),
        3,
    );
    assert!(summary.has_data_cache_parallel_scheduler_livelock_diagnostics());
    assert_eq!(summary.full_system_progress_transition_count(), 12);
    assert_eq!(summary.full_system_livelock_diagnostic_count(), 5);
    assert!(summary.has_full_system_diagnostics());

    let productive_retry_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_livelock_diagnostics(5, 0)
        .with_data_cache_parallel_scheduler_livelock_diagnostics(7, 0);
    assert_eq!(
        productive_retry_summary.full_system_progress_transition_count(),
        12,
    );
    assert_eq!(
        productive_retry_summary.full_system_livelock_diagnostic_count(),
        0,
    );
    assert!(!productive_retry_summary.has_parallel_scheduler_livelock_diagnostics());
    assert!(!productive_retry_summary.has_data_cache_parallel_scheduler_livelock_diagnostics());
    assert!(!productive_retry_summary.has_full_system_diagnostics());
}

#[test]
fn workload_result_preserves_livelock_diagnostic_records() {
    let shared_subject = component("shared-progress-loop");
    let scheduler_diagnostic = livelock_diagnostic(
        shared_subject.clone(),
        1,
        [(LivelockTransitionKind::ProtocolRetry, 0)],
    );
    let data_cache_diagnostic = livelock_diagnostic(
        shared_subject.clone(),
        1,
        [(LivelockTransitionKind::MessageRetry, 3)],
    );
    let full_system_diagnostic = livelock_diagnostic(
        shared_subject.clone(),
        2,
        [
            (LivelockTransitionKind::ProtocolRetry, 0),
            (LivelockTransitionKind::MessageRetry, 3),
        ],
    );
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_livelock_diagnostic_records(1, [scheduler_diagnostic.clone()])
        .with_data_cache_parallel_scheduler_livelock_diagnostic_records(
            1,
            [data_cache_diagnostic.clone()],
        )
        .with_full_system_livelock_diagnostic_records([full_system_diagnostic.clone()]);

    assert_eq!(
        summary.parallel_scheduler_livelock_diagnostics(),
        &[scheduler_diagnostic],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_livelock_diagnostics(),
        &[data_cache_diagnostic],
    );
    assert_eq!(
        summary.full_system_livelock_diagnostics(),
        vec![full_system_diagnostic],
    );
    assert_eq!(summary.parallel_scheduler_livelock_diagnostic_count(), 1);
    assert_eq!(
        summary.data_cache_parallel_scheduler_livelock_diagnostic_count(),
        1,
    );
    assert_eq!(summary.full_system_livelock_diagnostic_count(), 1);
}

#[test]
fn workload_manifest_records_livelock_transition_threshold() {
    let thresholded =
        expected_clean_with_livelock_threshold(WorkloadParallelDiagnosticScope::FullSystem, 3);
    assert_eq!(thresholded.livelock_transition_threshold(), Some(3));

    let manifest = rem6_workload::WorkloadManifest::builder(id("livelock-threshold"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_clean_parallel_diagnostics(thresholded)
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    assert_eq!(
        plan.expected_clean_parallel_diagnostics()[0].livelock_transition_threshold(),
        Some(3),
    );

    let unthresholded =
        rem6_workload::WorkloadManifest::builder(id("livelock-threshold"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::FullSystem,
            ))
            .unwrap()
            .build()
            .unwrap();
    assert_ne!(manifest.identity(), unthresholded.identity());

    assert_eq!(
        WorkloadExpectedCleanParallelDiagnostics::new(WorkloadParallelDiagnosticScope::FullSystem)
            .with_livelock_transition_threshold(0)
            .unwrap_err(),
        WorkloadError::ZeroExpectedLivelockTransitionThreshold {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_livelock_dirty_parallel_diagnostics() {
    let plan = replay_plan()
        .add_expected_clean_parallel_diagnostics(expected_clean(
            WorkloadParallelDiagnosticScope::FullSystem,
        ))
        .unwrap();

    let dirty_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_livelock_diagnostic_records(
            4,
            [livelock_diagnostic(
                component("cpu-progress-loop"),
                1,
                [(LivelockTransitionKind::ProtocolRetry, 0)],
            )],
        )
        .with_data_cache_parallel_scheduler_livelock_diagnostic_records(
            3,
            [livelock_diagnostic(
                component("cache-progress-loop"),
                1,
                [(LivelockTransitionKind::MessageRetry, 3)],
            )],
        );
    let dirty_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(dirty_summary);

    assert_eq!(
        plan.verify_result(&dirty_result).unwrap_err(),
        WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            wait_for_edge_count: 0,
            deadlock_diagnostic_count: 0,
            livelock_diagnostic_count: 2,
            livelock_subjects: vec![
                "component:cache-progress-loop".to_string(),
                "component:cpu-progress-loop".to_string(),
            ],
        },
    );
}

#[test]
fn workload_clean_data_cache_diagnostics_include_data_cache_livelock() {
    let plan = replay_plan()
        .add_expected_clean_parallel_diagnostics(expected_clean(
            WorkloadParallelDiagnosticScope::DataCache,
        ))
        .unwrap();

    let dirty_summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_livelock_diagnostics(4, 1)
        .with_data_cache_parallel_scheduler_livelock_diagnostics(3, 2);
    let dirty_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(dirty_summary);

    assert_eq!(
        plan.verify_result(&dirty_result).unwrap_err(),
        WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
            scope: WorkloadParallelDiagnosticScope::DataCache,
            wait_for_edge_count: 0,
            deadlock_diagnostic_count: 0,
            livelock_diagnostic_count: 2,
            livelock_subjects: Vec::new(),
        },
    );
}

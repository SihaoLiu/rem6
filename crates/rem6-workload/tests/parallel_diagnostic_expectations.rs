use rem6_boot::BootImage;
use rem6_kernel::{WaitForEdgeKind, WaitForNode};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedCleanParallelDiagnostics,
    WorkloadExpectedParallelWaitForBlockedNodeWindow, WorkloadExpectedParallelWaitForEdgeKindCount,
    WorkloadExpectedParallelWaitForEdgeKindWindow, WorkloadExpectedParallelWaitForTargetNodeWindow,
    WorkloadId, WorkloadParallelDiagnosticScope, WorkloadParallelExecutionSummary,
    WorkloadReplayPlan, WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
    WorkloadWaitForBlockedNodeWindow, WorkloadWaitForEdgeKindWindow,
    WorkloadWaitForTargetNodeWindow,
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

fn expected_clean(
    scope: WorkloadParallelDiagnosticScope,
) -> WorkloadExpectedCleanParallelDiagnostics {
    WorkloadExpectedCleanParallelDiagnostics::new(scope)
}

fn expected_wait_kind(
    scope: WorkloadParallelDiagnosticScope,
    kind: WaitForEdgeKind,
    minimum_edge_count: usize,
) -> WorkloadExpectedParallelWaitForEdgeKindCount {
    WorkloadExpectedParallelWaitForEdgeKindCount::new(scope, kind, minimum_edge_count).unwrap()
}

fn expected_wait_kind_window(
    scope: WorkloadParallelDiagnosticScope,
    kind: WaitForEdgeKind,
    edge_count: usize,
    first_tick: u64,
    last_tick: u64,
) -> WorkloadExpectedParallelWaitForEdgeKindWindow {
    WorkloadExpectedParallelWaitForEdgeKindWindow::new(
        scope, kind, edge_count, first_tick, last_tick,
    )
    .unwrap()
}

fn wait_resource(value: &str) -> WaitForNode {
    WaitForNode::resource(value).unwrap()
}

fn expected_wait_target_window(
    scope: WorkloadParallelDiagnosticScope,
    node: WaitForNode,
    edge_count: usize,
    first_tick: u64,
    last_tick: u64,
) -> WorkloadExpectedParallelWaitForTargetNodeWindow {
    WorkloadExpectedParallelWaitForTargetNodeWindow::new(
        scope, node, edge_count, first_tick, last_tick,
    )
    .unwrap()
}

fn expected_wait_blocked_window(
    scope: WorkloadParallelDiagnosticScope,
    node: WaitForNode,
    edge_count: usize,
    first_tick: u64,
    last_tick: u64,
) -> WorkloadExpectedParallelWaitForBlockedNodeWindow {
    WorkloadExpectedParallelWaitForBlockedNodeWindow::new(
        scope, node, edge_count, first_tick, last_tick,
    )
    .unwrap()
}

#[test]
fn workload_manifest_records_clean_parallel_diagnostic_expectations() {
    let resource = expected_clean(WorkloadParallelDiagnosticScope::Resource);
    let data_cache = expected_clean(WorkloadParallelDiagnosticScope::DataCache);
    let compute = expected_clean(WorkloadParallelDiagnosticScope::Compute);
    let dma = expected_clean(WorkloadParallelDiagnosticScope::Dma);
    let full_system = expected_clean(WorkloadParallelDiagnosticScope::FullSystem);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(full_system)
            .unwrap()
            .add_expected_clean_parallel_diagnostics(dma)
            .unwrap()
            .add_expected_clean_parallel_diagnostics(resource)
            .unwrap()
            .add_expected_clean_parallel_diagnostics(compute)
            .unwrap()
            .add_expected_clean_parallel_diagnostics(data_cache)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_clean_parallel_diagnostics(),
        &[resource, data_cache, compute, dma, full_system],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_clean_parallel_diagnostics(),
        manifest.expected_clean_parallel_diagnostics(),
    );
    assert_eq!(
        WorkloadParallelDiagnosticScope::Resource.as_str(),
        "resource"
    );
    assert_eq!(
        WorkloadParallelDiagnosticScope::DataCache.as_str(),
        "data-cache"
    );
    assert_eq!(WorkloadParallelDiagnosticScope::Compute.as_str(), "compute");
    assert_eq!(WorkloadParallelDiagnosticScope::Dma.as_str(), "dma");
    assert_eq!(
        WorkloadParallelDiagnosticScope::FullSystem.as_str(),
        "full-system",
    );

    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(WorkloadParallelExecutionSummary::default());
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_result_reports_merged_full_system_deadlocks_as_diagnostics() {
    let summary =
        WorkloadParallelExecutionSummary::default().with_merged_full_system_deadlock_diagnostics(3);

    assert_eq!(summary.merged_full_system_deadlock_diagnostic_count(), 3);
    assert_eq!(summary.full_system_deadlock_diagnostic_count(), 3);
    assert!(summary.has_full_system_diagnostics());
}

#[test]
fn workload_manifest_records_parallel_wait_for_edge_kind_expectations() {
    let resource_queue = expected_wait_kind(
        WorkloadParallelDiagnosticScope::Resource,
        WaitForEdgeKind::Queue,
        2,
    );
    let full_system_barrier = expected_wait_kind(
        WorkloadParallelDiagnosticScope::FullSystem,
        WaitForEdgeKind::Barrier,
        1,
    );
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-wait-kind-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_count(full_system_barrier)
    .unwrap()
    .add_expected_parallel_wait_for_edge_kind_count(resource_queue)
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(
        manifest.expected_parallel_wait_for_edge_kind_counts(),
        &[resource_queue, full_system_barrier],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_wait_for_edge_kind_counts(),
        manifest.expected_parallel_wait_for_edge_kind_counts(),
    );
}

#[test]
fn workload_manifest_records_parallel_wait_for_edge_kind_window_expectations() {
    let resource_queue = expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::Resource,
        WaitForEdgeKind::Queue,
        2,
        3,
        9,
    );
    let full_system_barrier = expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        WaitForEdgeKind::Barrier,
        1,
        7,
        7,
    );
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-wait-kind-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_window(full_system_barrier)
    .unwrap()
    .add_expected_parallel_wait_for_edge_kind_window(resource_queue)
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(
        manifest.expected_parallel_wait_for_edge_kind_windows(),
        &[resource_queue, full_system_barrier],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_wait_for_edge_kind_windows(),
        manifest.expected_parallel_wait_for_edge_kind_windows(),
    );
}

#[test]
fn workload_manifest_records_parallel_wait_for_target_node_window_expectations() {
    let resource_queue = expected_wait_target_window(
        WorkloadParallelDiagnosticScope::Resource,
        wait_resource("fabric.queue.0"),
        2,
        3,
        9,
    );
    let full_system_bank = expected_wait_target_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        wait_resource("dram.bank.0"),
        1,
        7,
        7,
    );
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_target_node_window(full_system_bank.clone())
    .unwrap()
    .add_expected_parallel_wait_for_target_node_window(resource_queue.clone())
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(
        manifest.expected_parallel_wait_for_target_node_windows(),
        &[resource_queue, full_system_bank],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_wait_for_target_node_windows(),
        manifest.expected_parallel_wait_for_target_node_windows(),
    );
}

#[test]
fn workload_manifest_records_parallel_wait_for_blocked_node_window_expectations() {
    let resource_port = expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::Resource,
        wait_resource("fabric.port.0"),
        2,
        3,
        9,
    );
    let full_system_core = expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        wait_resource("cpu.core.0"),
        1,
        7,
        7,
    );
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("manifest-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_blocked_node_window(full_system_core.clone())
    .unwrap()
    .add_expected_parallel_wait_for_blocked_node_window(resource_port.clone())
    .unwrap()
    .build()
    .unwrap();

    assert_eq!(
        manifest.expected_parallel_wait_for_blocked_node_windows(),
        &[resource_port, full_system_core],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_parallel_wait_for_blocked_node_windows(),
        manifest.expected_parallel_wait_for_blocked_node_windows(),
    );
}

#[test]
fn workload_manifest_identity_changes_with_parallel_wait_for_edge_kind_counts() {
    let base = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-kind-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let queue = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-kind-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_count(expected_wait_kind(
        WorkloadParallelDiagnosticScope::Resource,
        WaitForEdgeKind::Queue,
        1,
    ))
    .unwrap()
    .build()
    .unwrap();
    let barrier = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-kind-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_count(expected_wait_kind(
        WorkloadParallelDiagnosticScope::Resource,
        WaitForEdgeKind::Barrier,
        1,
    ))
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(base.identity(), queue.identity());
    assert_ne!(queue.identity(), barrier.identity());
}

#[test]
fn workload_manifest_identity_changes_with_parallel_wait_for_edge_kind_windows() {
    let base = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-kind-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let queue = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-kind-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_window(expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::Resource,
        WaitForEdgeKind::Queue,
        1,
        3,
        3,
    ))
    .unwrap()
    .build()
    .unwrap();
    let wider_queue = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-kind-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_window(expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::Resource,
        WaitForEdgeKind::Queue,
        1,
        3,
        8,
    ))
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(base.identity(), queue.identity());
    assert_ne!(queue.identity(), wider_queue.identity());
}

#[test]
fn workload_manifest_identity_changes_with_parallel_wait_for_target_node_windows() {
    let base = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let queue = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_target_node_window(expected_wait_target_window(
        WorkloadParallelDiagnosticScope::Resource,
        wait_resource("fabric.queue.0"),
        1,
        3,
        3,
    ))
    .unwrap()
    .build()
    .unwrap();
    let credit = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_target_node_window(expected_wait_target_window(
        WorkloadParallelDiagnosticScope::Resource,
        wait_resource("fabric.credit.0"),
        1,
        3,
        3,
    ))
    .unwrap()
    .build()
    .unwrap();
    let wider_queue = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_target_node_window(expected_wait_target_window(
        WorkloadParallelDiagnosticScope::Resource,
        wait_resource("fabric.queue.0"),
        1,
        3,
        8,
    ))
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(base.identity(), queue.identity());
    assert_ne!(queue.identity(), credit.identity());
    assert_ne!(queue.identity(), wider_queue.identity());
}

#[test]
fn workload_manifest_identity_changes_with_parallel_wait_for_blocked_node_windows() {
    let base = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let core = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_blocked_node_window(expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::Resource,
        wait_resource("cpu.core.0"),
        1,
        3,
        3,
    ))
    .unwrap()
    .build()
    .unwrap();
    let dma = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_blocked_node_window(expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::Resource,
        wait_resource("gpu.dma.engine.0"),
        1,
        3,
        3,
    ))
    .unwrap()
    .build()
    .unwrap();
    let wider_core = rem6_workload::WorkloadManifest::builder(
        id("identity-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_blocked_node_window(expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::Resource,
        wait_resource("cpu.core.0"),
        1,
        3,
        8,
    ))
    .unwrap()
    .build()
    .unwrap();

    assert_ne!(base.identity(), core.identity());
    assert_ne!(core.identity(), dma.identity());
    assert_ne!(core.identity(), wider_core.identity());
}

#[test]
fn workload_replay_plan_verifies_parallel_wait_for_edge_kind_counts() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("verify-wait-kind-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_wait_for_edge_kind_count(expected_wait_kind(
                WorkloadParallelDiagnosticScope::FullSystem,
                WaitForEdgeKind::Barrier,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelDiagnosticSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_counts([(WaitForEdgeKind::Barrier, 1)]);
    let underactive_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive_result).unwrap_err(),
        WorkloadError::ExpectedParallelWaitForEdgeKindCountBelowMinimum {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            kind: WaitForEdgeKind::Barrier,
            minimum_edge_count: 2,
            actual_edge_count: 1,
        },
    );

    let satisfied_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_counts([(WaitForEdgeKind::Barrier, 1)])
        .with_resource_wait_for_edge_kind_counts(
            [(WaitForEdgeKind::Barrier, 1)],
            [(WaitForEdgeKind::Queue, 3)],
        );
    let satisfied_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(satisfied_summary);
    plan.verify_result(&satisfied_result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_inconsistent_wait_for_diagnostic_summary() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("inconsistent-wait-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_parallel_wait_for_edge_kind_count(expected_wait_kind(
                WorkloadParallelDiagnosticScope::FullSystem,
                WaitForEdgeKind::Barrier,
                2,
            ))
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let inconsistent_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_windows([WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Barrier,
            2,
            4,
            12,
        )])
        .with_data_cache_wait_for_edge_kind_counts([(WaitForEdgeKind::Barrier, 1)]);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(inconsistent_summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelWaitForEdgeKindWindowSummary {
            scope: WorkloadParallelDiagnosticScope::DataCache,
            kind: WaitForEdgeKind::Barrier,
            edge_kind_count: 1,
            window_edge_count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_inconsistent_wait_for_edge_count_summary() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("inconsistent-wait-edge-count-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_count(expected_wait_kind(
        WorkloadParallelDiagnosticScope::FullSystem,
        WaitForEdgeKind::Barrier,
        2,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let inconsistent_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_counts([(WaitForEdgeKind::Barrier, 2)])
        .with_data_cache_diagnostics(1, 0);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(inconsistent_summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelWaitForEdgeCountSummary {
            scope: WorkloadParallelDiagnosticScope::DataCache,
            wait_for_edge_count: 1,
            evidence_edge_count: 2,
        },
    );
}

#[test]
fn workload_replay_plan_uses_explicit_full_system_wait_for_edge_kind_counts() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("explicit-full-system-wait-kind-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_count(expected_wait_kind(
        WorkloadParallelDiagnosticScope::FullSystem,
        WaitForEdgeKind::Queue,
        5,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_counts([(WaitForEdgeKind::Queue, 2)])
        .with_full_system_wait_for_edge_kind_counts([
            (WaitForEdgeKind::Queue, 5),
            (WaitForEdgeKind::Resource, 3),
        ]);
    assert_eq!(
        summary.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        5,
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Resource),
        3,
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 8);

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_weaker_explicit_full_system_wait_for_edge_kind_counts() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("weak-full-system-wait-kind-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_count(expected_wait_kind(
        WorkloadParallelDiagnosticScope::FullSystem,
        WaitForEdgeKind::Queue,
        4,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_counts([(WaitForEdgeKind::Queue, 4)])
        .with_full_system_wait_for_edge_kind_counts([(WaitForEdgeKind::Queue, 3)]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelWaitForEdgeKindCountMergeSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            kind: WaitForEdgeKind::Queue,
            merged_edge_count: 3,
            scoped_edge_count: 4,
        },
    );
}

#[test]
fn workload_replay_plan_uses_explicit_full_system_wait_for_edge_kind_windows() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("explicit-full-system-wait-kind-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_window(expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        WaitForEdgeKind::Barrier,
        5,
        4,
        20,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_windows([WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Barrier,
            2,
            6,
            10,
        )])
        .with_full_system_wait_for_edge_kind_windows([
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Barrier, 5, 4, 20),
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Resource, 3, 7, 9),
        ]);
    assert_eq!(
        summary.full_system_wait_for_edge_kind_window(WaitForEdgeKind::Barrier),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Barrier,
            5,
            4,
            20,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Barrier),
        5,
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Resource),
        3,
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 8);

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_weaker_explicit_full_system_wait_for_edge_kind_windows() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("weak-full-system-wait-kind-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_window(expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        WaitForEdgeKind::Barrier,
        4,
        4,
        12,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_windows([WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Barrier,
            4,
            4,
            12,
        )])
        .with_full_system_wait_for_edge_kind_windows([WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Barrier,
            4,
            5,
            11,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelWaitForEdgeKindWindowMergeSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            kind: WaitForEdgeKind::Barrier,
            merged_edge_count: 4,
            scoped_edge_count: 4,
            merged_first_tick: 5,
            scoped_first_tick: 4,
            merged_last_tick: 11,
            scoped_last_tick: 12,
        },
    );
}

#[test]
fn workload_replay_plan_uses_explicit_full_system_wait_for_blocked_node_windows() {
    let blocked = wait_resource("fabric.queue.0");
    let scheduler = wait_resource("full-system.scheduler");
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("explicit-full-system-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_blocked_node_window(expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        blocked.clone(),
        5,
        4,
        20,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_blocked_node_windows([WorkloadWaitForBlockedNodeWindow::new(
            blocked.clone(),
            2,
            6,
            10,
        )])
        .with_full_system_wait_for_blocked_node_windows([
            WorkloadWaitForBlockedNodeWindow::new(blocked.clone(), 5, 4, 20),
            WorkloadWaitForBlockedNodeWindow::new(scheduler.clone(), 3, 7, 9),
        ]);
    assert_eq!(
        summary.full_system_wait_for_blocked_node_window(&blocked),
        Some(WorkloadWaitForBlockedNodeWindow::new(
            blocked.clone(),
            5,
            4,
            20,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_blocked_node_window(&scheduler),
        Some(WorkloadWaitForBlockedNodeWindow::new(scheduler, 3, 7, 9)),
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 8);

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_weaker_explicit_full_system_wait_for_blocked_node_windows() {
    let blocked = wait_resource("fabric.queue.0");
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("weak-full-system-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_blocked_node_window(expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        blocked.clone(),
        4,
        4,
        12,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let summary =
        WorkloadParallelExecutionSummary::default()
            .with_data_cache_wait_for_blocked_node_windows([WorkloadWaitForBlockedNodeWindow::new(
                blocked.clone(),
                4,
                4,
                12,
            )])
            .with_full_system_wait_for_blocked_node_windows([
                WorkloadWaitForBlockedNodeWindow::new(blocked.clone(), 4, 5, 11),
            ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelWaitForBlockedNodeWindowMergeSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            node: blocked,
            merged_edge_count: 4,
            scoped_edge_count: 4,
            merged_first_tick: 5,
            scoped_first_tick: 4,
            merged_last_tick: 11,
            scoped_last_tick: 12,
        },
    );
}

#[test]
fn workload_replay_plan_uses_explicit_full_system_wait_for_target_node_windows() {
    let target = wait_resource("dram.bank.0");
    let scheduler = wait_resource("full-system.scheduler");
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("explicit-full-system-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_target_node_window(expected_wait_target_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        target.clone(),
        5,
        4,
        20,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_target_node_windows([WorkloadWaitForTargetNodeWindow::new(
            target.clone(),
            2,
            6,
            10,
        )])
        .with_full_system_wait_for_target_node_windows([
            WorkloadWaitForTargetNodeWindow::new(target.clone(), 5, 4, 20),
            WorkloadWaitForTargetNodeWindow::new(scheduler.clone(), 3, 7, 9),
        ]);
    assert_eq!(
        summary.full_system_wait_for_target_node_window(&target),
        Some(WorkloadWaitForTargetNodeWindow::new(
            target.clone(),
            5,
            4,
            20,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_target_node_window(&scheduler),
        Some(WorkloadWaitForTargetNodeWindow::new(scheduler, 3, 7, 9)),
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 8);

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_weaker_explicit_full_system_wait_for_target_node_windows() {
    let target = wait_resource("dram.bank.0");
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("weak-full-system-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_target_node_window(expected_wait_target_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        target.clone(),
        4,
        4,
        12,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_target_node_windows([WorkloadWaitForTargetNodeWindow::new(
            target.clone(),
            4,
            4,
            12,
        )])
        .with_full_system_wait_for_target_node_windows([WorkloadWaitForTargetNodeWindow::new(
            target.clone(),
            4,
            5,
            11,
        )]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);

    assert_eq!(
        plan.verify_result(&result).unwrap_err(),
        WorkloadError::InvalidParallelWaitForTargetNodeWindowMergeSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            node: target,
            merged_edge_count: 4,
            scoped_edge_count: 4,
            merged_first_tick: 5,
            scoped_first_tick: 4,
            merged_last_tick: 11,
            scoped_last_tick: 12,
        },
    );
}

#[test]
fn workload_replay_plan_verifies_parallel_wait_for_edge_kind_windows() {
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("verify-wait-kind-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_window(expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        WaitForEdgeKind::Barrier,
        2,
        4,
        12,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelDiagnosticSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
        },
    );

    let underactive_summary =
        WorkloadParallelExecutionSummary::default().with_data_cache_wait_for_edge_kind_windows([
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Barrier, 1, 4, 4),
        ]);
    let underactive_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive_result).unwrap_err(),
        WorkloadError::ExpectedParallelWaitForEdgeKindWindowMismatch {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            kind: WaitForEdgeKind::Barrier,
            expected_edge_count: 2,
            actual_edge_count: 1,
            expected_first_tick: 4,
            actual_first_tick: Some(4),
            expected_last_tick: 12,
            actual_last_tick: Some(4),
        },
    );

    let satisfied_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_windows([WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Barrier,
            1,
            4,
            6,
        )])
        .with_resource_wait_for_edge_kind_windows(
            [WorkloadWaitForEdgeKindWindow::new(
                WaitForEdgeKind::Barrier,
                1,
                8,
                12,
            )],
            [],
        );
    let satisfied_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(satisfied_summary);
    plan.verify_result(&satisfied_result).unwrap();
}

#[test]
fn workload_replay_plan_verifies_parallel_wait_for_target_node_windows() {
    let target = wait_resource("fabric.queue.0");
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("verify-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_target_node_window(expected_wait_target_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        target.clone(),
        2,
        4,
        12,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelDiagnosticSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
        },
    );

    let underactive_summary =
        WorkloadParallelExecutionSummary::default().with_data_cache_wait_for_target_node_windows([
            WorkloadWaitForTargetNodeWindow::new(target.clone(), 1, 4, 4),
        ]);
    let underactive_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive_result).unwrap_err(),
        WorkloadError::ExpectedParallelWaitForTargetNodeWindowMismatch {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            node: target.clone(),
            expected_edge_count: 2,
            actual_edge_count: 1,
            expected_first_tick: 4,
            actual_first_tick: Some(4),
            expected_last_tick: 12,
            actual_last_tick: Some(4),
        },
    );

    let satisfied_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_target_node_windows([WorkloadWaitForTargetNodeWindow::new(
            target.clone(),
            1,
            4,
            6,
        )])
        .with_resource_wait_for_target_node_windows(
            [WorkloadWaitForTargetNodeWindow::new(
                target.clone(),
                1,
                8,
                12,
            )],
            [],
        );
    let satisfied_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(satisfied_summary);
    plan.verify_result(&satisfied_result).unwrap();
}

#[test]
fn workload_replay_plan_verifies_parallel_wait_for_blocked_node_windows() {
    let blocked = wait_resource("cpu.core.0");
    let manifest = rem6_workload::WorkloadManifest::builder(
        id("verify-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_blocked_node_window(expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::FullSystem,
        blocked.clone(),
        2,
        4,
        12,
    ))
    .unwrap()
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelDiagnosticSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_blocked_node_windows([WorkloadWaitForBlockedNodeWindow::new(
            blocked.clone(),
            1,
            4,
            4,
        )]);
    let underactive_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive_result).unwrap_err(),
        WorkloadError::ExpectedParallelWaitForBlockedNodeWindowMismatch {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            node: blocked.clone(),
            expected_edge_count: 2,
            actual_edge_count: 1,
            expected_first_tick: 4,
            actual_first_tick: Some(4),
            expected_last_tick: 12,
            actual_last_tick: Some(4),
        },
    );

    let satisfied_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_blocked_node_windows([WorkloadWaitForBlockedNodeWindow::new(
            blocked.clone(),
            1,
            4,
            6,
        )])
        .with_resource_wait_for_blocked_node_windows(
            [WorkloadWaitForBlockedNodeWindow::new(
                blocked.clone(),
                1,
                8,
                12,
            )],
            [],
        );
    let satisfied_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(satisfied_summary);
    plan.verify_result(&satisfied_result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_wait_for_edge_kind_windows() {
    assert_eq!(
        WorkloadExpectedParallelWaitForEdgeKindWindow::new(
            WorkloadParallelDiagnosticScope::Resource,
            WaitForEdgeKind::Queue,
            0,
            3,
            7,
        )
        .unwrap_err(),
        WorkloadError::ZeroExpectedParallelWaitForEdgeKindWindow {
            scope: WorkloadParallelDiagnosticScope::Resource,
            kind: WaitForEdgeKind::Queue,
        },
    );
    assert_eq!(
        WorkloadExpectedParallelWaitForEdgeKindWindow::new(
            WorkloadParallelDiagnosticScope::Resource,
            WaitForEdgeKind::Queue,
            1,
            9,
            7,
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelWaitForEdgeKindWindow {
            scope: WorkloadParallelDiagnosticScope::Resource,
            kind: WaitForEdgeKind::Queue,
            first_tick: 9,
            last_tick: 7,
        },
    );

    let duplicate = rem6_workload::WorkloadManifest::builder(
        id("duplicate-wait-kind-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_edge_kind_window(expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::DataCache,
        WaitForEdgeKind::Protocol,
        1,
        3,
        3,
    ))
    .unwrap()
    .add_expected_parallel_wait_for_edge_kind_window(expected_wait_kind_window(
        WorkloadParallelDiagnosticScope::DataCache,
        WaitForEdgeKind::Protocol,
        1,
        4,
        4,
    ))
    .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelWaitForEdgeKindWindow {
            scope: WorkloadParallelDiagnosticScope::DataCache,
            kind: WaitForEdgeKind::Protocol,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_wait_for_target_node_windows() {
    let target = wait_resource("fabric.queue.0");
    assert_eq!(
        WorkloadExpectedParallelWaitForTargetNodeWindow::new(
            WorkloadParallelDiagnosticScope::Resource,
            target.clone(),
            0,
            3,
            7,
        )
        .unwrap_err(),
        WorkloadError::ZeroExpectedParallelWaitForTargetNodeWindow {
            scope: WorkloadParallelDiagnosticScope::Resource,
            node: target.clone(),
        },
    );
    assert_eq!(
        WorkloadExpectedParallelWaitForTargetNodeWindow::new(
            WorkloadParallelDiagnosticScope::Resource,
            target.clone(),
            1,
            9,
            7,
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelWaitForTargetNodeWindow {
            scope: WorkloadParallelDiagnosticScope::Resource,
            node: target.clone(),
            first_tick: 9,
            last_tick: 7,
        },
    );

    let duplicate = rem6_workload::WorkloadManifest::builder(
        id("duplicate-wait-target-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_target_node_window(expected_wait_target_window(
        WorkloadParallelDiagnosticScope::DataCache,
        target.clone(),
        1,
        3,
        3,
    ))
    .unwrap()
    .add_expected_parallel_wait_for_target_node_window(expected_wait_target_window(
        WorkloadParallelDiagnosticScope::DataCache,
        target.clone(),
        1,
        4,
        4,
    ))
    .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelWaitForTargetNodeWindow {
            scope: WorkloadParallelDiagnosticScope::DataCache,
            node: target,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_parallel_wait_for_blocked_node_windows() {
    let blocked = wait_resource("cpu.core.0");
    assert_eq!(
        WorkloadExpectedParallelWaitForBlockedNodeWindow::new(
            WorkloadParallelDiagnosticScope::Resource,
            blocked.clone(),
            0,
            3,
            7,
        )
        .unwrap_err(),
        WorkloadError::ZeroExpectedParallelWaitForBlockedNodeWindow {
            scope: WorkloadParallelDiagnosticScope::Resource,
            node: blocked.clone(),
        },
    );
    assert_eq!(
        WorkloadExpectedParallelWaitForBlockedNodeWindow::new(
            WorkloadParallelDiagnosticScope::Resource,
            blocked.clone(),
            1,
            9,
            7,
        )
        .unwrap_err(),
        WorkloadError::InvalidExpectedParallelWaitForBlockedNodeWindow {
            scope: WorkloadParallelDiagnosticScope::Resource,
            node: blocked.clone(),
            first_tick: 9,
            last_tick: 7,
        },
    );

    let duplicate = rem6_workload::WorkloadManifest::builder(
        id("duplicate-wait-blocked-window-diagnostics"),
        boot_image(),
    )
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .add_expected_parallel_wait_for_blocked_node_window(expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::DataCache,
        blocked.clone(),
        1,
        3,
        3,
    ))
    .unwrap()
    .add_expected_parallel_wait_for_blocked_node_window(expected_wait_blocked_window(
        WorkloadParallelDiagnosticScope::DataCache,
        blocked.clone(),
        1,
        4,
        4,
    ))
    .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedParallelWaitForBlockedNodeWindow {
            scope: WorkloadParallelDiagnosticScope::DataCache,
            node: blocked,
        },
    );
}

#[test]
fn workload_manifest_identity_changes_with_clean_parallel_diagnostics() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let resource =
        rem6_workload::WorkloadManifest::builder(id("identity-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::Resource,
            ))
            .unwrap()
            .build()
            .unwrap();
    let full_system =
        rem6_workload::WorkloadManifest::builder(id("identity-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::FullSystem,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), resource.identity());
    assert_ne!(resource.identity(), full_system.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_dirty_parallel_diagnostics() {
    let manifest = rem6_workload::WorkloadManifest::builder(id("dirty-diagnostics"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_clean_parallel_diagnostics(expected_clean(
            WorkloadParallelDiagnosticScope::FullSystem,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelDiagnosticSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
        },
    );

    let dirty_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_diagnostics(2, 0)
        .with_resource_diagnostics(0, 1, 0, 0)
        .with_merged_full_system_deadlock_diagnostics(6)
        .with_gpu_compute_diagnostics(3, 0)
        .with_accelerator_dma_diagnostics(0, 4);
    let dirty_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(dirty_summary);
    assert_eq!(
        plan.verify_result(&dirty_result).unwrap_err(),
        WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            wait_for_edge_count: 5,
            deadlock_diagnostic_count: 10,
            livelock_diagnostic_count: 0,
            livelock_subjects: Vec::new(),
        },
    );
}

#[test]
fn workload_replay_plan_rejects_incomplete_resource_deadlock_merge() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("dirty-resource-merge"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_clean_parallel_diagnostics(expected_clean(
            WorkloadParallelDiagnosticScope::Resource,
        ))
        .unwrap();

    let dirty_summary = WorkloadParallelExecutionSummary::default()
        .with_resource_diagnostics(0, 2, 0, 1)
        .with_merged_resource_deadlock_diagnostics(1);
    let dirty_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(dirty_summary);

    assert_eq!(
        plan.verify_result(&dirty_result).unwrap_err(),
        WorkloadError::InvalidParallelDeadlockMergeSummary {
            scope: WorkloadParallelDiagnosticScope::Resource,
            merged_diagnostic_count: 1,
            scoped_diagnostic_count: 3,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_incomplete_full_system_deadlock_merge() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("dirty-full-system-merge"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_clean_parallel_diagnostics(expected_clean(
            WorkloadParallelDiagnosticScope::FullSystem,
        ))
        .unwrap();

    let dirty_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_diagnostics(0, 2)
        .with_resource_diagnostics(0, 1, 0, 0)
        .with_merged_full_system_deadlock_diagnostics(1);
    let dirty_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(dirty_summary);

    assert_eq!(
        plan.verify_result(&dirty_result).unwrap_err(),
        WorkloadError::InvalidParallelDeadlockMergeSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            merged_diagnostic_count: 1,
            scoped_diagnostic_count: 3,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_clean_parallel_diagnostics() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("duplicate-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::DataCache,
            ))
            .unwrap()
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::DataCache,
            ))
            .unwrap_err();
    assert_eq!(
        manifest,
        WorkloadError::DuplicateExpectedCleanParallelDiagnostics {
            scope: WorkloadParallelDiagnosticScope::DataCache,
        },
    );
}

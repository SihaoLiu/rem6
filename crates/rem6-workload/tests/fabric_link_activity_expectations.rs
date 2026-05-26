use rem6_boot::BootImage;
use rem6_fabric::{FabricLinkActivity, FabricLinkId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedFabricLinkActivity, WorkloadId,
    WorkloadParallelExecutionSummary, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn link(value: &str) -> FabricLinkId {
    FabricLinkId::new(value).unwrap()
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
        rem6_workload::WorkloadManifest::builder(id("fabric-link-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_link_activity(
    link: &str,
    minimum_transfer_count: usize,
    minimum_active_virtual_network_count: usize,
    minimum_queue_delay_ticks: u64,
    minimum_contended_virtual_network_count: usize,
) -> WorkloadExpectedFabricLinkActivity {
    WorkloadExpectedFabricLinkActivity::new(
        self::link(link),
        minimum_transfer_count,
        minimum_active_virtual_network_count,
        minimum_queue_delay_ticks,
        minimum_contended_virtual_network_count,
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn link_activity(
    link: &str,
    active_virtual_network_count: usize,
    transfer_count: usize,
    byte_count: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    contended_virtual_network_count: usize,
    first_tick: u64,
    last_tick: u64,
) -> FabricLinkActivity {
    FabricLinkActivity::new(
        self::link(link),
        active_virtual_network_count,
        transfer_count,
        byte_count,
        occupied_ticks,
        queue_delay_ticks,
        max_queue_delay_ticks,
        contended_virtual_network_count,
        first_tick,
        last_tick,
    )
}

#[test]
fn workload_manifest_records_fabric_link_activity_expectations() {
    let mesh_a = expected_link_activity("mesh_a", 7, 2, 11, 1);
    let mesh_b = expected_link_activity("mesh_b", 3, 1, 0, 0);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-fabric-link-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(mesh_b.clone())
            .unwrap()
            .add_expected_fabric_link_activity(mesh_a.clone())
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_fabric_link_activity(),
        &[mesh_a.clone(), mesh_b.clone()],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_fabric_link_activity(),
        manifest.expected_fabric_link_activity(),
    );

    let summary = WorkloadParallelExecutionSummary::default().with_fabric_link_activities([
        link_activity("mesh_a", 2, 7, 224, 31, 11, 8, 1, 0, 12),
        link_activity("mesh_b", 1, 3, 64, 9, 0, 0, 0, 2, 6),
    ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_fabric_link_activity() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let mesh_a =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(expected_link_activity("mesh_a", 1, 1, 0, 0))
            .unwrap()
            .build()
            .unwrap();
    let mesh_b =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(expected_link_activity("mesh_b", 1, 1, 0, 0))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), mesh_a.identity());
    assert_ne!(mesh_a.identity(), mesh_b.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_fabric_link_activity() {
    let plan = replay_plan()
        .add_expected_fabric_link_activity(expected_link_activity("mesh_a", 4, 2, 6, 1))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingFabricLinkActivitySummary {
            link: link("mesh_a"),
            minimum_transfer_count: 4,
            minimum_active_virtual_network_count: 2,
            minimum_queue_delay_ticks: 6,
            minimum_contended_virtual_network_count: 1,
            required_first_tick: None,
            required_last_tick: None,
        },
    );

    let missing_link = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(WorkloadParallelExecutionSummary::default());
    assert_eq!(
        plan.verify_result(&missing_link).unwrap_err(),
        WorkloadError::MissingFabricLinkActivitySummary {
            link: link("mesh_a"),
            minimum_transfer_count: 4,
            minimum_active_virtual_network_count: 2,
            minimum_queue_delay_ticks: 6,
            minimum_contended_virtual_network_count: 1,
            required_first_tick: None,
            required_last_tick: None,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_link_activities([link_activity("mesh_a", 1, 3, 96, 12, 5, 4, 0, 0, 8)]);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedFabricLinkActivityBelowMinimum {
            link: link("mesh_a"),
            minimum_transfer_count: 4,
            actual_transfer_count: 3,
            minimum_active_virtual_network_count: 2,
            actual_active_virtual_network_count: 1,
            minimum_queue_delay_ticks: 6,
            actual_queue_delay_ticks: 5,
            minimum_contended_virtual_network_count: 1,
            actual_contended_virtual_network_count: 0,
            required_first_tick: None,
            actual_first_tick: 0,
            required_last_tick: None,
            actual_last_tick: 8,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_fabric_link_activity_outside_required_window() {
    let expected = expected_link_activity("mesh_a", 4, 2, 6, 1)
        .with_required_tick_window(4, 16)
        .unwrap();
    assert_eq!(expected.required_tick_window(), Some((4, 16)));
    assert_eq!(expected.required_first_tick(), Some(4));
    assert_eq!(expected.required_last_tick(), Some(16));
    let plan = replay_plan()
        .add_expected_fabric_link_activity(expected)
        .unwrap();

    let satisfied_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_link_activities([link_activity("mesh_a", 2, 5, 160, 15, 9, 5, 1, 4, 16)]);
    let satisfied = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(satisfied_summary);
    plan.verify_result(&satisfied).unwrap();

    let late_start_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_link_activities([link_activity("mesh_a", 2, 5, 160, 15, 9, 5, 1, 5, 18)]);
    let late_start = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(late_start_summary);
    assert_eq!(
        plan.verify_result(&late_start).unwrap_err(),
        WorkloadError::ExpectedFabricLinkActivityBelowMinimum {
            link: link("mesh_a"),
            minimum_transfer_count: 4,
            actual_transfer_count: 5,
            minimum_active_virtual_network_count: 2,
            actual_active_virtual_network_count: 2,
            minimum_queue_delay_ticks: 6,
            actual_queue_delay_ticks: 9,
            minimum_contended_virtual_network_count: 1,
            actual_contended_virtual_network_count: 1,
            required_first_tick: Some(4),
            actual_first_tick: 5,
            required_last_tick: Some(16),
            actual_last_tick: 18,
        },
    );

    let early_end_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_link_activities([link_activity("mesh_a", 2, 5, 160, 15, 9, 5, 1, 3, 15)]);
    let early_end = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(early_end_summary);
    assert_eq!(
        plan.verify_result(&early_end).unwrap_err(),
        WorkloadError::ExpectedFabricLinkActivityBelowMinimum {
            link: link("mesh_a"),
            minimum_transfer_count: 4,
            actual_transfer_count: 5,
            minimum_active_virtual_network_count: 2,
            actual_active_virtual_network_count: 2,
            minimum_queue_delay_ticks: 6,
            actual_queue_delay_ticks: 9,
            minimum_contended_virtual_network_count: 1,
            actual_contended_virtual_network_count: 1,
            required_first_tick: Some(4),
            actual_first_tick: 3,
            required_last_tick: Some(16),
            actual_last_tick: 15,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_overbudget_fabric_link_queue_delay() {
    let expected = expected_link_activity("mesh_a", 4, 2, 6, 1)
        .with_queue_delay_budget(9, 5)
        .unwrap();
    assert_eq!(expected.queue_delay_budget(), Some((9, 5)));
    assert_eq!(expected.maximum_queue_delay_ticks(), Some(9));
    assert_eq!(expected.maximum_max_queue_delay_ticks(), Some(5));
    let plan = replay_plan()
        .add_expected_fabric_link_activity(expected)
        .unwrap();

    let satisfied_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_link_activities([link_activity("mesh_a", 2, 5, 160, 15, 9, 5, 1, 3, 18)]);
    let satisfied = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(satisfied_summary);
    plan.verify_result(&satisfied).unwrap();

    let over_total_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_link_activities([link_activity("mesh_a", 2, 5, 160, 15, 10, 5, 1, 3, 18)]);
    let over_total = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(over_total_summary);
    assert_eq!(
        plan.verify_result(&over_total).unwrap_err(),
        WorkloadError::ExpectedFabricLinkActivityAboveMaximum {
            link: link("mesh_a"),
            maximum_queue_delay_ticks: 9,
            actual_queue_delay_ticks: 10,
            maximum_max_queue_delay_ticks: 5,
            actual_max_queue_delay_ticks: 5,
        },
    );

    let over_peak_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_link_activities([link_activity("mesh_a", 2, 5, 160, 15, 8, 6, 1, 3, 18)]);
    let over_peak = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(over_peak_summary);
    assert_eq!(
        plan.verify_result(&over_peak).unwrap_err(),
        WorkloadError::ExpectedFabricLinkActivityAboveMaximum {
            link: link("mesh_a"),
            maximum_queue_delay_ticks: 9,
            actual_queue_delay_ticks: 8,
            maximum_max_queue_delay_ticks: 5,
            actual_max_queue_delay_ticks: 6,
        },
    );
}

#[test]
fn workload_manifest_identity_changes_with_fabric_link_queue_budget() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-budget"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(expected_link_activity("mesh_a", 4, 2, 6, 1))
            .unwrap()
            .build()
            .unwrap();
    let budgeted =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-budget"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(
                expected_link_activity("mesh_a", 4, 2, 6, 1)
                    .with_queue_delay_budget(9, 5)
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
    let tighter =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-budget"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(
                expected_link_activity("mesh_a", 4, 2, 6, 1)
                    .with_queue_delay_budget(8, 5)
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), budgeted.identity());
    assert_ne!(budgeted.identity(), tighter.identity());
}

#[test]
fn workload_manifest_identity_changes_with_fabric_link_activity_window() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-window"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(expected_link_activity("mesh_a", 4, 2, 6, 1))
            .unwrap()
            .build()
            .unwrap();
    let windowed =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-window"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(
                expected_link_activity("mesh_a", 4, 2, 6, 1)
                    .with_required_tick_window(4, 16)
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();
    let shifted =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-link-window"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_link_activity(
                expected_link_activity("mesh_a", 4, 2, 6, 1)
                    .with_required_tick_window(5, 16)
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), windowed.identity());
    assert_ne!(windowed.identity(), shifted.identity());
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_fabric_link_activity() {
    let zero = WorkloadExpectedFabricLinkActivity::new(link("mesh_a"), 0, 0, 0, 0).unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedFabricLinkActivity {
            link: link("mesh_a"),
        },
    );

    let invalid_window = expected_link_activity("mesh_a", 1, 1, 0, 0)
        .with_required_tick_window(9, 7)
        .unwrap_err();
    assert_eq!(
        invalid_window,
        WorkloadError::InvalidExpectedFabricLinkActivityWindow {
            link: link("mesh_a"),
            first_tick: 9,
            last_tick: 7,
        },
    );

    let invalid_budget = expected_link_activity("mesh_a", 1, 1, 0, 0)
        .with_queue_delay_budget(4, 5)
        .unwrap_err();
    assert_eq!(
        invalid_budget,
        WorkloadError::InvalidExpectedFabricLinkActivityQueueDelayBudget {
            link: link("mesh_a"),
            maximum_queue_delay_ticks: 4,
            maximum_max_queue_delay_ticks: 5,
        },
    );

    let duplicate = replay_plan()
        .add_expected_fabric_link_activity(expected_link_activity("mesh_a", 1, 1, 0, 0))
        .unwrap()
        .add_expected_fabric_link_activity(expected_link_activity("mesh_a", 2, 1, 0, 0))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedFabricLinkActivity {
            link: link("mesh_a"),
        },
    );
}

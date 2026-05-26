use rem6_boot::BootImage;
use rem6_fabric::{FabricLaneActivity, FabricLinkId, VirtualNetworkId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedFabricLaneActivity, WorkloadId,
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

fn vn(value: u16) -> VirtualNetworkId {
    VirtualNetworkId::new(value)
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
        rem6_workload::WorkloadManifest::builder(id("fabric-lane-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_lane_activity(
    link: &str,
    virtual_network: u16,
    minimum_transfer_count: usize,
    minimum_byte_count: u64,
    minimum_occupied_ticks: u64,
    minimum_queue_delay_ticks: u64,
) -> WorkloadExpectedFabricLaneActivity {
    WorkloadExpectedFabricLaneActivity::new(
        self::link(link),
        vn(virtual_network),
        minimum_transfer_count,
        minimum_byte_count,
        minimum_occupied_ticks,
        minimum_queue_delay_ticks,
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn lane_activity(
    link: &str,
    virtual_network: u16,
    transfer_count: usize,
    byte_count: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    first_tick: u64,
    last_tick: u64,
) -> FabricLaneActivity {
    FabricLaneActivity::new(
        self::link(link),
        vn(virtual_network),
        transfer_count,
        byte_count,
        occupied_ticks,
        queue_delay_ticks,
        max_queue_delay_ticks,
        first_tick,
        last_tick,
    )
}

#[test]
fn workload_manifest_records_fabric_lane_activity_expectations() {
    let control = expected_lane_activity("mesh_a", 1, 7, 224, 31, 11);
    let data = expected_lane_activity("mesh_b", 2, 3, 64, 9, 0);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-fabric-lane-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_lane_activity(data.clone())
            .unwrap()
            .add_expected_fabric_lane_activity(control.clone())
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_fabric_lane_activity(),
        &[control.clone(), data.clone()],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_fabric_lane_activity(),
        manifest.expected_fabric_lane_activity(),
    );

    let summary = WorkloadParallelExecutionSummary::default().with_fabric_lane_activities([
        lane_activity("mesh_a", 1, 7, 224, 31, 11, 8, 0, 12),
        lane_activity("mesh_b", 2, 3, 64, 9, 0, 0, 2, 6),
    ]);
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_fabric_lane_activity() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-lane-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let control =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-lane-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_lane_activity(expected_lane_activity("mesh_a", 1, 1, 8, 1, 0))
            .unwrap()
            .build()
            .unwrap();
    let data =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-lane-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_lane_activity(expected_lane_activity("mesh_a", 2, 1, 8, 1, 0))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), control.identity());
    assert_ne!(control.identity(), data.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_fabric_lane_activity() {
    let plan = replay_plan()
        .add_expected_fabric_lane_activity(expected_lane_activity("mesh_a", 1, 4, 128, 12, 6))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingFabricLaneActivitySummary {
            link: link("mesh_a"),
            virtual_network: vn(1),
            minimum_transfer_count: 4,
            minimum_byte_count: 128,
            minimum_occupied_ticks: 12,
            minimum_queue_delay_ticks: 6,
        },
    );

    let missing_lane = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(WorkloadParallelExecutionSummary::default());
    assert_eq!(
        plan.verify_result(&missing_lane).unwrap_err(),
        WorkloadError::MissingFabricLaneActivitySummary {
            link: link("mesh_a"),
            virtual_network: vn(1),
            minimum_transfer_count: 4,
            minimum_byte_count: 128,
            minimum_occupied_ticks: 12,
            minimum_queue_delay_ticks: 6,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_lane_activities([lane_activity("mesh_a", 1, 3, 96, 11, 5, 4, 0, 8)]);
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::ExpectedFabricLaneActivityBelowMinimum {
            link: link("mesh_a"),
            virtual_network: vn(1),
            minimum_transfer_count: 4,
            actual_transfer_count: 3,
            minimum_byte_count: 128,
            actual_byte_count: 96,
            minimum_occupied_ticks: 12,
            actual_occupied_ticks: 11,
            minimum_queue_delay_ticks: 6,
            actual_queue_delay_ticks: 5,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_fabric_lane_activity() {
    let zero =
        WorkloadExpectedFabricLaneActivity::new(link("mesh_a"), vn(1), 0, 0, 0, 0).unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedFabricLaneActivity {
            link: link("mesh_a"),
            virtual_network: vn(1),
        },
    );

    let duplicate = replay_plan()
        .add_expected_fabric_lane_activity(expected_lane_activity("mesh_a", 1, 1, 8, 1, 0))
        .unwrap()
        .add_expected_fabric_lane_activity(expected_lane_activity("mesh_a", 1, 2, 16, 1, 0))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedFabricLaneActivity {
            link: link("mesh_a"),
            virtual_network: vn(1),
        },
    );
}

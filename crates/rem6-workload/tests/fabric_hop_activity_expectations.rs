use rem6_boot::BootImage;
use rem6_fabric::{
    FabricHopActivity, FabricLinkId, FabricModel, FabricPacket, FabricPacketId, FabricPath,
    FabricPathHop, VirtualNetworkId,
};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedFabricHopActivity, WorkloadId, WorkloadParallelExecutionSummary,
    WorkloadReplayPlan, WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
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
        rem6_workload::WorkloadManifest::builder(id("fabric-hop-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    WorkloadReplayPlan::from_manifest(&manifest).unwrap()
}

fn expected_hop_activity(
    hop_index: usize,
    link: &str,
    virtual_network: u16,
    minimum_transfer_count: usize,
    minimum_byte_count: u64,
    minimum_occupied_ticks: u64,
    minimum_queue_delay_ticks: u64,
) -> WorkloadExpectedFabricHopActivity {
    WorkloadExpectedFabricHopActivity::new(
        hop_index,
        self::link(link),
        vn(virtual_network),
        minimum_transfer_count,
        minimum_byte_count,
        minimum_occupied_ticks,
        minimum_queue_delay_ticks,
    )
    .unwrap()
}

fn packet(id: u64, bytes: u64, virtual_network: u16) -> FabricPacket {
    FabricPacket::new(
        FabricPacketId::new(id),
        bytes,
        VirtualNetworkId::new(virtual_network),
    )
    .unwrap()
}

fn path(hops: impl IntoIterator<Item = FabricPathHop>) -> FabricPath {
    FabricPath::new(hops).unwrap()
}

fn multihop_activities() -> Vec<FabricHopActivity> {
    let mut fabric = FabricModel::new();
    fabric
        .transmit(
            5,
            packet(7, 16, 1),
            path([
                FabricPathHop::new(link("cpu_to_router"), 2, 8).unwrap(),
                FabricPathHop::new(link("router_to_mem"), 3, 4)
                    .unwrap()
                    .with_virtual_network(vn(3)),
            ]),
        )
        .unwrap();
    fabric.hop_activities()
}

#[test]
fn workload_manifest_records_fabric_hop_activity_expectations() {
    let second_hop = expected_hop_activity(1, "router_to_mem", 3, 1, 16, 4, 0)
        .with_required_tick_window(9, 16)
        .unwrap();
    let first_hop = expected_hop_activity(0, "cpu_to_router", 1, 1, 16, 2, 0);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-fabric-hop-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_hop_activity(second_hop.clone())
            .unwrap()
            .add_expected_fabric_hop_activity(first_hop.clone())
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_fabric_hop_activity(),
        &[first_hop.clone(), second_hop.clone()],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_fabric_hop_activity(),
        manifest.expected_fabric_hop_activity(),
    );

    let summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_hop_activities(multihop_activities());
    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_parallel_execution_summary(summary);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_fabric_hop_activity() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-hop-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let first_hop =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-hop-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_hop_activity(expected_hop_activity(
                0,
                "cpu_to_router",
                1,
                1,
                16,
                2,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();
    let second_hop =
        rem6_workload::WorkloadManifest::builder(id("identity-fabric-hop-activity"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_fabric_hop_activity(expected_hop_activity(
                1,
                "router_to_mem",
                3,
                1,
                16,
                4,
                0,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), first_hop.identity());
    assert_ne!(first_hop.identity(), second_hop.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_underactive_fabric_hop_activity() {
    let plan = replay_plan()
        .add_expected_fabric_hop_activity(expected_hop_activity(1, "router_to_mem", 3, 2, 32, 8, 0))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingFabricHopActivitySummary {
            hop_index: 1,
            link: link("router_to_mem"),
            virtual_network: vn(3),
            minimum_transfer_count: 2,
            minimum_byte_count: 32,
            minimum_occupied_ticks: 8,
            minimum_queue_delay_ticks: 0,
            required_first_tick: None,
            required_last_tick: None,
        },
    );

    let missing_hop = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(WorkloadParallelExecutionSummary::default());
    assert_eq!(
        plan.verify_result(&missing_hop).unwrap_err(),
        WorkloadError::MissingFabricHopActivitySummary {
            hop_index: 1,
            link: link("router_to_mem"),
            virtual_network: vn(3),
            minimum_transfer_count: 2,
            minimum_byte_count: 32,
            minimum_occupied_ticks: 8,
            minimum_queue_delay_ticks: 0,
            required_first_tick: None,
            required_last_tick: None,
        },
    );

    let underactive_summary = WorkloadParallelExecutionSummary::default()
        .with_fabric_hop_activities(multihop_activities());
    let underactive = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(underactive_summary);
    assert_eq!(
        plan.verify_result(&underactive).unwrap_err(),
        WorkloadError::expected_fabric_hop_activity_below_minimum(
            1,
            link("router_to_mem"),
            vn(3),
            2,
            1,
            32,
            16,
            8,
            4,
            0,
            0,
            None,
            9,
            None,
            16,
        ),
    );
}

#[test]
fn workload_replay_plan_rejects_invalid_or_duplicate_fabric_hop_activity() {
    let zero = WorkloadExpectedFabricHopActivity::new(1, link("router_to_mem"), vn(3), 0, 0, 0, 0)
        .unwrap_err();
    assert_eq!(
        zero,
        WorkloadError::ZeroExpectedFabricHopActivity {
            hop_index: 1,
            link: link("router_to_mem"),
            virtual_network: vn(3),
        },
    );

    let invalid_window = expected_hop_activity(1, "router_to_mem", 3, 1, 16, 4, 0)
        .with_required_tick_window(17, 16)
        .unwrap_err();
    assert_eq!(
        invalid_window,
        WorkloadError::InvalidExpectedFabricHopActivityWindow {
            hop_index: 1,
            link: link("router_to_mem"),
            virtual_network: vn(3),
            first_tick: 17,
            last_tick: 16,
        },
    );

    let duplicate = replay_plan()
        .add_expected_fabric_hop_activity(expected_hop_activity(1, "router_to_mem", 3, 1, 16, 4, 0))
        .unwrap()
        .add_expected_fabric_hop_activity(expected_hop_activity(1, "router_to_mem", 3, 2, 32, 8, 0))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateExpectedFabricHopActivity {
            hop_index: 1,
            link: link("router_to_mem"),
            virtual_network: vn(3),
        },
    );
}

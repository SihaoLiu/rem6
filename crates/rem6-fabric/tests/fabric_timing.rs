use rem6_fabric::{
    FabricActivityProfile, FabricError, FabricLaneActivity, FabricLinkActivity, FabricLinkId,
    FabricModel, FabricPacket, FabricPacketId, FabricPath, FabricPathHop, FabricRouterId,
    FabricRouterInputVcSnapshot, FabricRouterOutputPortSnapshot, FabricRouterStage, FabricSnapshot,
    FabricVirtualNetworkActivity, VirtualNetworkId,
};
use rem6_kernel::{ClockDomain, Cycles, WaitForEdgeKind, WaitForNode};

fn packet(id: u64, bytes: u64, virtual_network: u16) -> FabricPacket {
    FabricPacket::new(
        FabricPacketId::new(id),
        bytes,
        VirtualNetworkId::new(virtual_network),
    )
    .unwrap()
}

fn link(name: &str) -> FabricLinkId {
    FabricLinkId::new(name).unwrap()
}

fn router(name: &str) -> FabricRouterId {
    FabricRouterId::new(name).unwrap()
}

fn path(hops: impl IntoIterator<Item = FabricPathHop>) -> FabricPath {
    FabricPath::new(hops).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn lane_activity(
    link: &str,
    virtual_network: u16,
    transfers: usize,
    bytes: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    first_tick: u64,
    last_tick: u64,
) -> FabricLaneActivity {
    FabricLaneActivity::new(
        self::link(link),
        VirtualNetworkId::new(virtual_network),
        transfers,
        bytes,
        occupied_ticks,
        queue_delay_ticks,
        max_queue_delay_ticks,
        first_tick,
        last_tick,
    )
}

#[test]
fn fabric_serializes_packets_on_shared_link_deterministically() {
    let mut fabric = FabricModel::new();
    let shared = link("mesh_x0");
    let route = path([FabricPathHop::new(shared.clone(), 2, 8).unwrap()]);
    let requests = [
        (packet(11, 16, 0), route.clone()),
        (packet(10, 16, 0), route.clone()),
    ];

    let transfers = fabric.transmit_batch(0, requests).unwrap();

    assert_eq!(transfers[0].packet().id(), FabricPacketId::new(10));
    assert_eq!(transfers[0].arrival_tick(), 4);
    assert_eq!(transfers[0].hops()[0].start_tick(), 0);
    assert_eq!(transfers[0].hops()[0].depart_tick(), 2);
    assert_eq!(transfers[1].packet().id(), FabricPacketId::new(11));
    assert_eq!(transfers[1].arrival_tick(), 6);
    assert_eq!(transfers[1].hops()[0].start_tick(), 2);
    assert_eq!(transfers[1].hops()[0].depart_tick(), 4);

    let later = fabric
        .transmit(1, packet(12, 16, 0), route.clone())
        .unwrap();
    assert_eq!(later.arrival_tick(), 8);
    assert_eq!(later.hops()[0].start_tick(), 4);
}

#[test]
fn fabric_serial_link_timing_uses_declared_clock_domain() {
    let mut fabric = FabricModel::new();
    let fast_clock = ClockDomain::new(1).unwrap();
    let slow_clock = ClockDomain::new(4).unwrap();
    let fast_link = link("serial_fast");
    let slow_link = link("serial_slow");
    let fast_route =
        path([
            FabricPathHop::serial_link(fast_link.clone(), fast_clock, Cycles::new(3), 4, 8)
                .unwrap(),
        ]);
    let slow_route =
        path([
            FabricPathHop::serial_link(slow_link.clone(), slow_clock, Cycles::new(3), 4, 8)
                .unwrap(),
        ]);

    let fast = fabric.transmit(7, packet(20, 64, 0), fast_route).unwrap();
    let slow = fabric.transmit(7, packet(21, 64, 0), slow_route).unwrap();

    assert_eq!(fast.hops()[0].serialization_ticks(), 16);
    assert_eq!(fast.hops()[0].depart_tick(), 23);
    assert_eq!(fast.arrival_tick(), 26);
    assert_eq!(slow.hops()[0].serialization_ticks(), 64);
    assert_eq!(slow.hops()[0].depart_tick(), 71);
    assert_eq!(slow.arrival_tick(), 83);
    assert_eq!(
        fabric
            .lane_activity(&fast_link, VirtualNetworkId::new(0))
            .unwrap()
            .flit_count(),
        16
    );
    assert_eq!(
        fabric
            .lane_activity(&slow_link, VirtualNetworkId::new(0))
            .unwrap()
            .flit_count(),
        16
    );
}

#[test]
fn fabric_serial_link_bits_per_nanosecond_converts_through_clock_domain() {
    let mut fabric = FabricModel::new();
    let fast_clock = ClockDomain::new(1).unwrap();
    let slow_clock = ClockDomain::new(4).unwrap();
    let fast_route = path([FabricPathHop::serial_link_bits_per_nanosecond(
        link("serial_ns_fast"),
        fast_clock,
        Cycles::new(3),
        4,
        8,
        1,
    )
    .unwrap()]);
    let slow_route = path([FabricPathHop::serial_link_bits_per_nanosecond(
        link("serial_ns_slow"),
        slow_clock,
        Cycles::new(3),
        4,
        8,
        1,
    )
    .unwrap()]);

    let fast = fabric.transmit(7, packet(22, 64, 0), fast_route).unwrap();
    let slow = fabric.transmit(7, packet(23, 64, 0), slow_route).unwrap();

    assert_eq!(fast.hops()[0].serialization_ticks(), 16);
    assert_eq!(fast.hops()[0].depart_tick(), 23);
    assert_eq!(fast.arrival_tick(), 26);
    assert_eq!(slow.hops()[0].serialization_ticks(), 16);
    assert_eq!(slow.hops()[0].depart_tick(), 23);
    assert_eq!(slow.arrival_tick(), 35);
}

#[test]
fn fabric_link_activity_merge_window_preserves_unique_virtual_network_coverage() {
    let first_lanes = [
        lane_activity("mesh_merge_link", 1, 2, 32, 4, 0, 0, 0, 5),
        lane_activity("mesh_merge_link", 2, 1, 16, 2, 7, 7, 1, 9),
    ];
    let second_lanes = [
        lane_activity("mesh_merge_link", 2, 3, 48, 6, 11, 11, 10, 18),
        lane_activity("mesh_merge_link", 3, 1, 16, 2, 0, 0, 12, 20),
    ];
    let first = FabricLinkActivity::from_lanes(first_lanes.iter())
        .into_iter()
        .next()
        .unwrap();
    let second = FabricLinkActivity::from_lanes(second_lanes.iter())
        .into_iter()
        .next()
        .unwrap();

    let merged = first.merge_window(second);

    assert_eq!(merged.active_virtual_network_count(), 3);
    assert_eq!(merged.contended_virtual_network_count(), 1);
    assert_eq!(merged.transfer_count(), 7);
    assert_eq!(merged.byte_count(), 112);
    assert_eq!(merged.occupied_ticks(), 14);
    assert_eq!(merged.queue_delay_ticks(), 18);
    assert_eq!(merged.max_queue_delay_ticks(), 11);
    assert_eq!(merged.first_tick(), 0);
    assert_eq!(merged.last_tick(), 20);
}

#[test]
fn fabric_virtual_network_activity_merge_window_preserves_unique_lane_coverage() {
    let first_lanes = [
        lane_activity("mesh_merge_a", 4, 2, 32, 4, 0, 0, 0, 5),
        lane_activity("mesh_merge_b", 4, 1, 16, 2, 7, 7, 1, 9),
    ];
    let second_lanes = [
        lane_activity("mesh_merge_b", 4, 3, 48, 6, 11, 11, 10, 18),
        lane_activity("mesh_merge_c", 4, 1, 16, 2, 0, 0, 12, 20),
    ];
    let first = FabricVirtualNetworkActivity::from_lanes(first_lanes.iter())
        .into_iter()
        .next()
        .unwrap();
    let second = FabricVirtualNetworkActivity::from_lanes(second_lanes.iter())
        .into_iter()
        .next()
        .unwrap();

    let merged = first.merge_window(second);

    assert_eq!(merged.active_lane_count(), 3);
    assert_eq!(merged.contended_lane_count(), 1);
    assert_eq!(merged.transfer_count(), 7);
    assert_eq!(merged.byte_count(), 112);
    assert_eq!(merged.occupied_ticks(), 14);
    assert_eq!(merged.queue_delay_ticks(), 18);
    assert_eq!(merged.max_queue_delay_ticks(), 11);
    assert_eq!(merged.first_tick(), 0);
    assert_eq!(merged.last_tick(), 20);
}

#[test]
fn fabric_aggregate_activity_equality_uses_public_summary_values() {
    let lanes = [
        lane_activity("mesh_public_eq", 6, 2, 32, 4, 7, 7, 0, 5).with_flit_count(8),
        lane_activity("mesh_public_eq", 7, 1, 16, 2, 0, 0, 6, 9).with_flit_count(4),
    ];
    let link_summary = FabricLinkActivity::from_lanes(lanes.iter())
        .into_iter()
        .next()
        .unwrap();
    let count_only_link =
        FabricLinkActivity::new(link("mesh_public_eq"), 2, 3, 48, 6, 7, 7, 1, 0, 9)
            .with_flit_count(12);
    assert_eq!(link_summary, count_only_link);
    assert_eq!(
        FabricActivityProfile::from_lanes(lanes.iter()),
        FabricActivityProfile::new(2, 3, 48, 6, 7, 7, 1).with_flit_count(12)
    );

    let vn_lanes = [
        lane_activity("mesh_public_eq_a", 8, 2, 32, 4, 7, 7, 0, 5).with_flit_count(8),
        lane_activity("mesh_public_eq_b", 8, 1, 16, 2, 0, 0, 6, 9).with_flit_count(4),
    ];
    let virtual_network_summary = FabricVirtualNetworkActivity::from_lanes(vn_lanes.iter())
        .into_iter()
        .next()
        .unwrap();
    let count_only_virtual_network =
        FabricVirtualNetworkActivity::new(VirtualNetworkId::new(8), 2, 3, 48, 6, 7, 7, 1, 0, 9)
            .with_flit_count(12);
    assert_eq!(virtual_network_summary, count_only_virtual_network);
}

#[test]
fn fabric_arbitrates_virtual_networks_on_shared_link() {
    let mut fabric = FabricModel::new();
    let shared = link("mesh_x0");
    let route = path([FabricPathHop::new(shared, 2, 8).unwrap()]);

    let transfers = fabric
        .transmit_batch(
            0,
            [
                (packet(1, 16, 1), route.clone()),
                (packet(2, 16, 2), route.clone()),
            ],
        )
        .unwrap();

    assert_eq!(transfers[0].arrival_tick(), 4);
    assert_eq!(transfers[0].hops()[0].start_tick(), 0);
    assert_eq!(transfers[1].arrival_tick(), 6);
    assert_eq!(transfers[1].hops()[0].start_tick(), 2);

    let shared_link_wait = fabric.wait_for_graph_at(1).snapshot();
    let packet = WaitForNode::transaction("fabric.packet.2").unwrap();
    let link = WaitForNode::resource("fabric.mesh_x0.link").unwrap();
    assert_eq!(shared_link_wait.edge_count(), 1);
    assert!(shared_link_wait.contains_edge(&packet, &link, WaitForEdgeKind::Queue));
}

#[test]
fn fabric_credit_depth_limits_in_flight_packets_per_virtual_network() {
    let mut fabric = FabricModel::new();
    let activity_start = fabric.mark_activity();
    let route = path([FabricPathHop::new(link("mesh_credit"), 10, 8)
        .unwrap()
        .with_credit_depth(2)
        .unwrap()]);

    let transfers = fabric
        .transmit_batch(
            0,
            [
                (packet(3, 8, 1), route.clone()),
                (packet(1, 8, 1), route.clone()),
                (packet(2, 8, 1), route.clone()),
            ],
        )
        .unwrap();

    assert_eq!(transfers[0].packet().id(), FabricPacketId::new(1));
    assert_eq!(transfers[0].hops()[0].start_tick(), 0);
    assert_eq!(transfers[0].hops()[0].depart_tick(), 1);
    assert_eq!(transfers[0].arrival_tick(), 11);
    assert_eq!(transfers[1].packet().id(), FabricPacketId::new(2));
    assert_eq!(transfers[1].hops()[0].start_tick(), 1);
    assert_eq!(transfers[1].hops()[0].depart_tick(), 2);
    assert_eq!(transfers[1].arrival_tick(), 12);
    assert_eq!(transfers[2].packet().id(), FabricPacketId::new(3));
    assert_eq!(transfers[2].hops()[0].start_tick(), 11);
    assert_eq!(transfers[2].hops()[0].ingress_tick(), 0);
    assert_eq!(transfers[2].hops()[0].depart_tick(), 12);
    assert_eq!(transfers[2].arrival_tick(), 22);

    let hop_activities = fabric.hop_activities_since(activity_start);
    assert_eq!(hop_activities.len(), transfers.len());
    for (activity, transfer) in hop_activities.iter().zip(&transfers) {
        assert_eq!(activity.timing(), &transfer.hops()[0]);
    }
    assert_eq!(hop_activities[2].credit_delay_ticks(), 9);

    let activity = fabric
        .lane_activity(&link("mesh_credit"), VirtualNetworkId::new(1))
        .unwrap();
    assert_eq!(activity.transfer_count(), 3);
    assert_eq!(activity.byte_count(), 24);
    assert_eq!(activity.occupied_ticks(), 3);
    assert_eq!(activity.queue_delay_ticks(), 12);
    assert_eq!(activity.max_queue_delay_ticks(), 11);
    assert_eq!(activity.credit_delay_ticks(), 9);
    assert_eq!(activity.max_credit_delay_ticks(), 9);
    assert_eq!(activity.first_tick(), 0);
    assert_eq!(activity.last_tick(), 22);
    assert!(activity.has_contention());
    assert_eq!(fabric.active_lane_count(), 1);
    assert_eq!(fabric.total_transfer_count(), 3);
    assert_eq!(fabric.total_queue_delay_ticks(), 12);

    let window = fabric.lane_activities_since(activity_start);
    assert_eq!(window, vec![activity]);

    let profile = fabric.activity_profile();
    assert_eq!(profile.active_lane_count(), 1);
    assert_eq!(profile.transfer_count(), 3);
    assert_eq!(profile.byte_count(), 24);
    assert_eq!(profile.occupied_ticks(), 3);
    assert_eq!(profile.queue_delay_ticks(), 12);
    assert_eq!(profile.max_queue_delay_ticks(), 11);
    assert_eq!(profile.credit_delay_ticks(), 9);
    assert_eq!(profile.max_credit_delay_ticks(), 9);
    assert_eq!(profile.contended_lane_count(), 1);
    assert!(profile.has_contention());
    assert!(!profile.is_empty());
    assert_eq!(fabric.activity_profile_since(activity_start), profile);

    fabric.clear_activity();
    assert!(fabric.activity_profile().is_empty());
    let later = fabric.transmit(0, packet(4, 8, 1), route).unwrap();
    assert_eq!(later.arrival_tick(), 23);
    assert_eq!(fabric.activity_profile().transfer_count(), 1);
}

#[test]
fn fabric_summarizes_activity_by_virtual_network() {
    let mut fabric = FabricModel::new();
    let shared = link("mesh_vn_summary");
    let route = path([FabricPathHop::new(shared, 2, 8)
        .unwrap()
        .with_credit_depth(1)
        .unwrap()]);

    fabric
        .transmit_batch(
            0,
            [
                (packet(3, 8, 1), route.clone()),
                (packet(1, 8, 1), route.clone()),
                (packet(2, 8, 2), route.clone()),
            ],
        )
        .unwrap();

    let activities = fabric.virtual_network_activities();
    assert_eq!(activities.len(), 2);
    assert_eq!(activities[0].virtual_network(), VirtualNetworkId::new(1));
    assert_eq!(activities[0].active_lane_count(), 1);
    assert_eq!(activities[0].transfer_count(), 2);
    assert_eq!(activities[0].byte_count(), 16);
    assert_eq!(activities[0].occupied_ticks(), 2);
    assert_eq!(activities[0].queue_delay_ticks(), 3);
    assert_eq!(activities[0].max_queue_delay_ticks(), 3);
    assert_eq!(activities[0].contended_lane_count(), 1);
    assert_eq!(activities[0].first_tick(), 0);
    assert_eq!(activities[0].last_tick(), 6);
    assert!(activities[0].has_contention());
    assert_eq!(activities[1].virtual_network(), VirtualNetworkId::new(2));
    assert_eq!(activities[1].transfer_count(), 1);
    assert_eq!(activities[1].queue_delay_ticks(), 1);
    assert_eq!(activities[1].max_queue_delay_ticks(), 1);
    assert_eq!(activities[1].contended_lane_count(), 1);
    assert!(activities[1].has_contention());
    assert_eq!(
        fabric
            .virtual_network_activity(VirtualNetworkId::new(1))
            .unwrap(),
        activities[0],
    );
}

#[test]
fn fabric_summarizes_activity_by_link_across_virtual_networks() {
    let mut fabric = FabricModel::new();
    let activity_start = fabric.mark_activity();
    let shared = link("mesh_link_summary");
    let route = path([FabricPathHop::new(shared.clone(), 2, 8)
        .unwrap()
        .with_credit_depth(1)
        .unwrap()]);

    fabric
        .transmit_batch(
            0,
            [
                (packet(3, 8, 1), route.clone()),
                (packet(1, 8, 1), route.clone()),
                (packet(2, 8, 2), route.clone()),
            ],
        )
        .unwrap();

    let activities = fabric.link_activities();
    assert_eq!(activities.len(), 1);
    assert_eq!(activities[0].link(), &shared);
    assert_eq!(activities[0].active_virtual_network_count(), 2);
    assert_eq!(activities[0].transfer_count(), 3);
    assert_eq!(activities[0].byte_count(), 24);
    assert_eq!(activities[0].occupied_ticks(), 3);
    assert_eq!(activities[0].queue_delay_ticks(), 4);
    assert_eq!(activities[0].max_queue_delay_ticks(), 3);
    assert_eq!(activities[0].contended_virtual_network_count(), 2);
    assert_eq!(activities[0].first_tick(), 0);
    assert_eq!(activities[0].last_tick(), 6);
    assert!(activities[0].has_contention());
    assert_eq!(fabric.link_activity(&shared).unwrap(), activities[0]);
    assert_eq!(fabric.link_activities_since(activity_start), activities);
}

#[test]
fn fabric_wait_for_graph_tracks_credit_blocked_packets_until_credit_returns() {
    let mut fabric = FabricModel::new();
    let route = path([FabricPathHop::new(link("mesh_wait_credit"), 10, 8)
        .unwrap()
        .with_credit_depth(2)
        .unwrap()]);

    fabric
        .transmit_batch(
            0,
            [
                (packet(3, 8, 1), route.clone()),
                (packet(1, 8, 1), route.clone()),
                (packet(2, 8, 1), route.clone()),
            ],
        )
        .unwrap();

    let packet_wait = WaitForNode::transaction("fabric.packet.3").unwrap();
    let lane = WaitForNode::resource("fabric.mesh_wait_credit.vn.1.lane").unwrap();
    let credit = WaitForNode::resource("fabric.mesh_wait_credit.vn.1.credit").unwrap();
    let queued_wait = fabric.wait_for_graph_at(1).snapshot();
    let active_wait = fabric.wait_for_graph_at(2).snapshot();

    assert_eq!(queued_wait.edge_count(), 1);
    assert_eq!(queued_wait.first_observed_tick(), Some(1));
    assert_eq!(queued_wait.last_observed_tick(), Some(1));
    assert!(queued_wait.contains_edge(&packet_wait, &lane, WaitForEdgeKind::Queue));
    assert_eq!(active_wait.edge_count(), 1);
    assert_eq!(active_wait.first_observed_tick(), Some(2));
    assert_eq!(active_wait.last_observed_tick(), Some(2));
    assert!(active_wait.contains_edge(&packet_wait, &credit, WaitForEdgeKind::Credit));
    assert_eq!(
        active_wait.dependencies(&packet_wait)[0].observation_count(),
        1
    );

    assert!(fabric.wait_for_graph_at(11).is_empty());
    assert!(fabric.wait_for_graph_at(22).is_empty());
}

#[test]
fn fabric_credit_depth_is_scoped_by_virtual_network() {
    let mut fabric = FabricModel::new();
    let route = path([FabricPathHop::new(link("mesh_credit_vn"), 10, 8)
        .unwrap()
        .with_credit_depth(1)
        .unwrap()]);

    let transfers = fabric
        .transmit_batch(
            0,
            [
                (packet(1, 8, 1), route.clone()),
                (packet(2, 8, 2), route.clone()),
            ],
        )
        .unwrap();

    assert_eq!(transfers[0].hops()[0].start_tick(), 0);
    assert_eq!(transfers[0].arrival_tick(), 11);
    assert_eq!(transfers[1].hops()[0].start_tick(), 1);
    assert_eq!(transfers[1].arrival_tick(), 12);
    assert_eq!(
        fabric
            .lane_activity(&link("mesh_credit_vn"), VirtualNetworkId::new(2))
            .unwrap()
            .credit_delay_ticks(),
        0
    );
}

#[test]
fn fabric_reports_credit_lane_state_in_deterministic_order() {
    let mut fabric = FabricModel::new();
    let mesh_a = path([FabricPathHop::new(link("mesh_a"), 10, 8)
        .unwrap()
        .with_credit_depth(2)
        .unwrap()]);
    let mesh_b = path([FabricPathHop::new(link("mesh_b"), 4, 8).unwrap()]);

    fabric
        .transmit_batch(
            0,
            [
                (packet(1, 8, 3), mesh_a.clone()),
                (packet(2, 8, 3), mesh_a),
                (packet(3, 8, 1), mesh_b),
            ],
        )
        .unwrap();

    let lanes = fabric.lane_snapshots();

    assert_eq!(lanes.len(), 2);
    assert_eq!(lanes[0].link().as_str(), "mesh_a");
    assert_eq!(lanes[0].virtual_network(), VirtualNetworkId::new(3));
    assert_eq!(lanes[0].next_available_tick(), 2);
    assert_eq!(lanes[0].credit_return_ticks(), &[11, 12]);
    assert_eq!(lanes[1].link().as_str(), "mesh_b");
    assert_eq!(lanes[1].virtual_network(), VirtualNetworkId::new(1));
    assert_eq!(lanes[1].next_available_tick(), 1);
    assert_eq!(lanes[1].credit_return_ticks(), &[]);
}

#[test]
fn fabric_restore_reinstates_lane_reservations() {
    let mut fabric = FabricModel::new();
    let route = path([FabricPathHop::new(link("mesh_restore"), 10, 8)
        .unwrap()
        .with_credit_depth(2)
        .unwrap()]);
    let alternate_vn_route = path([FabricPathHop::new(link("mesh_restore"), 10, 8)
        .unwrap()
        .with_credit_depth(2)
        .unwrap()]);

    fabric
        .transmit_batch(
            0,
            [
                (packet(1, 8, 1), route.clone()),
                (packet(2, 8, 1), route.clone()),
            ],
        )
        .unwrap();
    let snapshot = fabric.lane_snapshots();
    let mut expected = fabric.clone();
    let expected_transfer = expected
        .transmit(1, packet(3, 8, 1), route.clone())
        .unwrap();
    let mut expected_cross_vn = fabric.clone();
    let expected_cross_vn_transfer = expected_cross_vn
        .transmit(1, packet(4, 8, 2), alternate_vn_route.clone())
        .unwrap();
    assert_eq!(expected_cross_vn_transfer.hops()[0].start_tick(), 2);

    fabric.transmit(20, packet(9, 8, 1), route.clone()).unwrap();
    assert_ne!(fabric.lane_snapshots(), snapshot);

    fabric.restore_lane_snapshots(snapshot.clone()).unwrap();

    assert_eq!(fabric.lane_snapshots(), snapshot);
    let cross_vn_replayed = fabric
        .transmit(1, packet(4, 8, 2), alternate_vn_route)
        .unwrap();
    assert_eq!(cross_vn_replayed, expected_cross_vn_transfer);
    assert_eq!(fabric.lane_snapshots(), expected_cross_vn.lane_snapshots());

    fabric.restore_lane_snapshots(snapshot.clone()).unwrap();
    assert_eq!(fabric.lane_snapshots(), snapshot);
    let replayed = fabric.transmit(1, packet(3, 8, 1), route).unwrap();
    assert_eq!(replayed, expected_transfer);
    assert_eq!(fabric.lane_snapshots(), expected.lane_snapshots());
}

#[test]
fn fabric_restore_reinstates_router_stage_reservations() {
    let mut fabric = FabricModel::new();
    let router = router("restore_router");
    let route = path([FabricPathHop::new(link("restore_router.out"), 2, 8)
        .unwrap()
        .with_router_stage(FabricRouterStage::new(router.clone(), 0, 1, 0, 3).unwrap())]);

    fabric
        .transmit_batch(
            0,
            [
                (packet(1, 8, 1), route.clone()),
                (packet(2, 8, 1), route.clone()),
            ],
        )
        .unwrap();
    let snapshot = fabric.snapshot();
    assert_eq!(snapshot.router_input_vcs().len(), 1);
    assert_eq!(
        snapshot.router_input_vcs()[0].router().as_str(),
        "restore_router"
    );
    assert_eq!(snapshot.router_input_vcs()[0].input_port(), 0);
    assert_eq!(snapshot.router_input_vcs()[0].virtual_channel(), 0);
    assert_eq!(snapshot.router_input_vcs()[0].next_available_tick(), 6);
    assert_eq!(snapshot.router_output_ports().len(), 1);
    assert_eq!(
        snapshot.router_output_ports()[0].router().as_str(),
        "restore_router"
    );
    assert_eq!(snapshot.router_output_ports()[0].output_port(), 1);
    assert_eq!(snapshot.router_output_ports()[0].next_available_tick(), 6);
    let mut expected = fabric.clone();
    let expected_transfer = expected
        .transmit(1, packet(3, 8, 1), route.clone())
        .unwrap();
    assert_eq!(
        expected_transfer.hops()[0]
            .router()
            .unwrap()
            .queue_delay_ticks(),
        5
    );

    fabric.transmit(20, packet(9, 8, 1), route.clone()).unwrap();
    assert_ne!(fabric.snapshot(), snapshot);

    fabric.restore_snapshot(snapshot.clone()).unwrap();

    assert_eq!(fabric.snapshot(), snapshot);
    let replayed = fabric.transmit(1, packet(3, 8, 1), route).unwrap();
    assert_eq!(replayed, expected_transfer);
    assert_eq!(fabric.snapshot(), expected.snapshot());
}

#[test]
fn fabric_restore_rejects_duplicate_lane_snapshots() {
    let mut fabric = FabricModel::new();
    let route = path([FabricPathHop::new(link("mesh_duplicate"), 10, 8).unwrap()]);
    fabric.transmit(0, packet(1, 8, 2), route).unwrap();

    let mut snapshot = fabric.lane_snapshots();
    snapshot.push(snapshot[0].clone());

    assert_eq!(
        fabric.restore_lane_snapshots(snapshot).unwrap_err(),
        FabricError::DuplicateLaneSnapshot {
            link: link("mesh_duplicate"),
            virtual_network: VirtualNetworkId::new(2),
        }
    );
}

#[test]
fn fabric_restore_rejects_duplicate_router_snapshots() {
    let mut fabric = FabricModel::new();
    let duplicate_input = FabricRouterInputVcSnapshot::new(router("router_duplicate"), 0, 1, 7);
    let duplicate_input_snapshot = FabricSnapshot::new(
        Vec::new(),
        vec![duplicate_input.clone(), duplicate_input],
        Vec::new(),
    );

    assert_eq!(
        fabric
            .restore_snapshot(duplicate_input_snapshot)
            .unwrap_err(),
        FabricError::DuplicateRouterInputVcSnapshot {
            router: router("router_duplicate"),
            input_port: 0,
            virtual_channel: 1,
        }
    );

    let duplicate_output = FabricRouterOutputPortSnapshot::new(router("router_duplicate"), 2, 9);
    let duplicate_output_snapshot = FabricSnapshot::new(
        Vec::new(),
        Vec::new(),
        vec![duplicate_output.clone(), duplicate_output],
    );

    assert_eq!(
        fabric
            .restore_snapshot(duplicate_output_snapshot)
            .unwrap_err(),
        FabricError::DuplicateRouterOutputPortSnapshot {
            router: router("router_duplicate"),
            output_port: 2,
        }
    );
}

#[test]
fn fabric_restore_sorts_credit_return_ticks() {
    let mut fabric = FabricModel::new();
    fabric
        .restore_lane_snapshots([rem6_fabric::FabricLaneSnapshot::new(
            link("mesh_credit_order"),
            VirtualNetworkId::new(4),
            9,
            vec![30, 10, 20],
        )])
        .unwrap();

    let lanes = fabric.lane_snapshots();

    assert_eq!(lanes.len(), 1);
    assert_eq!(lanes[0].link().as_str(), "mesh_credit_order");
    assert_eq!(lanes[0].virtual_network(), VirtualNetworkId::new(4));
    assert_eq!(lanes[0].next_available_tick(), 9);
    assert_eq!(lanes[0].credit_return_ticks(), &[10, 20, 30]);
}

#[test]
fn fabric_pipelines_multi_hop_paths_by_link_occupancy() {
    let mut fabric = FabricModel::new();
    let route = path([
        FabricPathHop::new(link("xbar_to_mesh"), 2, 8).unwrap(),
        FabricPathHop::new(link("mesh_to_mem"), 3, 4).unwrap(),
    ]);

    let transfer = fabric.transmit(0, packet(7, 16, 0), route).unwrap();

    assert_eq!(transfer.arrival_tick(), 11);
    assert_eq!(transfer.hops()[0].start_tick(), 0);
    assert_eq!(transfer.hops()[0].serialization_ticks(), 2);
    assert_eq!(transfer.hops()[0].depart_tick(), 2);
    assert_eq!(transfer.hops()[0].arrival_tick(), 4);
    assert_eq!(transfer.hops()[1].start_tick(), 4);
    assert_eq!(transfer.hops()[1].serialization_ticks(), 4);
    assert_eq!(transfer.hops()[1].depart_tick(), 8);
    assert_eq!(transfer.hops()[1].arrival_tick(), 11);
}

#[test]
fn fabric_records_transfer_hop_activity_for_multihop_paths() {
    let mut fabric = FabricModel::new();
    let warmup = fabric
        .transmit(
            1,
            packet(6, 8, 9),
            path([FabricPathHop::new(link("activity_warmup"), 1, 8).unwrap()]),
        )
        .unwrap();
    let activity_start = fabric.mark_activity();
    let route = path([
        FabricPathHop::new(link("cpu_to_router"), 2, 8).unwrap(),
        FabricPathHop::new(link("router_to_mem"), 3, 4)
            .unwrap()
            .with_virtual_network(VirtualNetworkId::new(3)),
    ]);

    let transfer = fabric.transmit(5, packet(7, 16, 1), route).unwrap();

    let activities = fabric.hop_activities_since(activity_start);
    assert_eq!(activities.len(), transfer.hops().len());
    for (activity, timing) in activities.iter().zip(transfer.hops()) {
        assert_eq!(activity.timing(), timing);
    }
    assert_eq!(activities[0].packet(), FabricPacketId::new(7));
    assert_eq!(activities[0].hop_index(), 0);
    assert_eq!(activities[0].link(), &link("cpu_to_router"));
    assert_eq!(activities[0].virtual_network(), VirtualNetworkId::new(1));
    assert_eq!(activities[0].bytes(), 16);
    assert_eq!(activities[0].ingress_tick(), 5);
    assert_eq!(activities[0].start_tick(), 5);
    assert_eq!(activities[0].occupied_ticks(), 2);
    assert_eq!(activities[0].depart_tick(), 7);
    assert_eq!(activities[0].arrival_tick(), 9);
    assert_eq!(activities[0].queue_delay_ticks(), 0);
    assert_eq!(activities[1].packet(), FabricPacketId::new(7));
    assert_eq!(activities[1].hop_index(), 1);
    assert_eq!(activities[1].link(), &link("router_to_mem"));
    assert_eq!(activities[1].virtual_network(), VirtualNetworkId::new(3));
    assert_eq!(activities[1].bytes(), 16);
    assert_eq!(activities[1].ingress_tick(), 9);
    assert_eq!(activities[1].start_tick(), 9);
    assert_eq!(activities[1].occupied_ticks(), 4);
    assert_eq!(activities[1].depart_tick(), 13);
    assert_eq!(activities[1].arrival_tick(), 16);
    assert_eq!(activities[1].queue_delay_ticks(), 0);
    assert!(activities
        .iter()
        .all(|activity| activity.router().is_none()));
    assert_eq!(fabric.hop_activities_since(activity_start), activities);
    let retained = fabric.hop_activities();
    assert_eq!(retained.len(), warmup.hops().len() + activities.len());
    assert_eq!(retained[0].timing(), &warmup.hops()[0]);
    assert_eq!(&retained[1..], activities.as_slice());

    fabric.clear_activity();
    assert!(fabric.hop_activities().is_empty());
}

#[test]
fn fabric_path_hops_can_override_packet_virtual_network() {
    let mut fabric = FabricModel::new();
    let route = path([
        FabricPathHop::new(link("cpu_to_router"), 2, 8).unwrap(),
        FabricPathHop::new(link("router_to_mem"), 3, 8)
            .unwrap()
            .with_virtual_network(VirtualNetworkId::new(3)),
    ]);

    let transfer = fabric.transmit(0, packet(7, 16, 1), route).unwrap();

    assert_eq!(
        transfer.hops()[0].virtual_network(),
        VirtualNetworkId::new(1)
    );
    assert_eq!(
        transfer.hops()[1].virtual_network(),
        VirtualNetworkId::new(3)
    );
    assert!(fabric
        .lane_activity(&link("cpu_to_router"), VirtualNetworkId::new(1))
        .is_some());
    assert!(fabric
        .lane_activity(&link("router_to_mem"), VirtualNetworkId::new(3))
        .is_some());
    assert!(fabric
        .lane_activity(&link("router_to_mem"), VirtualNetworkId::new(1))
        .is_none());
}

#[test]
fn fabric_router_stage_serializes_input_virtual_channel_before_link() {
    let mut fabric = FabricModel::new();
    let router = router("router0");
    let route_a = path([FabricPathHop::new(link("router0.out0"), 2, 8)
        .unwrap()
        .with_router_stage(FabricRouterStage::new(router.clone(), 0, 1, 0, 3).unwrap())]);
    let route_b = path([FabricPathHop::new(link("router0.out1"), 2, 8)
        .unwrap()
        .with_router_stage(FabricRouterStage::new(router.clone(), 0, 2, 0, 3).unwrap())]);

    let transfers = fabric
        .transmit_batch(
            0,
            [(packet(11, 16, 0), route_b), (packet(10, 16, 0), route_a)],
        )
        .unwrap();

    assert_eq!(transfers[0].packet().id(), FabricPacketId::new(10));
    assert_eq!(transfers[0].arrival_tick(), 7);
    assert_eq!(transfers[0].hops()[0].start_tick(), 3);
    assert_eq!(transfers[0].hops()[0].depart_tick(), 5);
    let first_router = transfers[0].hops()[0].router().unwrap();
    assert_eq!(first_router.router().as_str(), "router0");
    assert_eq!(first_router.input_port(), 0);
    assert_eq!(first_router.output_port(), 1);
    assert_eq!(first_router.virtual_channel(), 0);
    assert_eq!(first_router.ready_tick(), 0);
    assert_eq!(first_router.start_tick(), 0);
    assert_eq!(first_router.latency_ticks(), 3);
    assert_eq!(first_router.depart_tick(), 3);
    assert_eq!(first_router.queue_delay_ticks(), 0);

    assert_eq!(transfers[1].packet().id(), FabricPacketId::new(11));
    assert_eq!(transfers[1].arrival_tick(), 10);
    assert_eq!(transfers[1].hops()[0].start_tick(), 6);
    assert_eq!(transfers[1].hops()[0].depart_tick(), 8);
    let second_router = transfers[1].hops()[0].router().unwrap();
    assert_eq!(second_router.router().as_str(), "router0");
    assert_eq!(second_router.input_port(), 0);
    assert_eq!(second_router.output_port(), 2);
    assert_eq!(second_router.virtual_channel(), 0);
    assert_eq!(second_router.ready_tick(), 0);
    assert_eq!(second_router.start_tick(), 3);
    assert_eq!(second_router.latency_ticks(), 3);
    assert_eq!(second_router.depart_tick(), 6);
    assert_eq!(second_router.queue_delay_ticks(), 3);

    let activities = fabric.hop_activities();
    assert_eq!(activities.len(), 2);
    assert_eq!(activities[0].timing(), &transfers[0].hops()[0]);
    assert_eq!(activities[1].timing(), &transfers[1].hops()[0]);
    assert_eq!(activities[0].ingress_tick(), 0);
    assert_eq!(activities[1].ingress_tick(), 0);
    assert_eq!(activities[0].link(), &link("router0.out0"));
    assert_eq!(activities[0].start_tick(), 3);
    assert_eq!(activities[0].queue_delay_ticks(), 0);
    assert_eq!(activities[0].router().unwrap().queue_delay_ticks(), 0);
    assert_eq!(activities[1].link(), &link("router0.out1"));
    assert_eq!(activities[1].start_tick(), 6);
    assert_eq!(activities[1].queue_delay_ticks(), 0);
    assert_eq!(activities[1].router().unwrap().queue_delay_ticks(), 3);
}

#[test]
fn fabric_router_stage_serializes_output_port_across_input_virtual_channels() {
    let mut fabric = FabricModel::new();
    let router = router("router1");
    let route_a = path([FabricPathHop::new(link("router1.out0a"), 1, 8)
        .unwrap()
        .with_router_stage(FabricRouterStage::new(router.clone(), 0, 9, 0, 2).unwrap())]);
    let route_b = path([FabricPathHop::new(link("router1.out0b"), 1, 8)
        .unwrap()
        .with_router_stage(FabricRouterStage::new(router.clone(), 1, 9, 1, 2).unwrap())]);

    let transfers = fabric
        .transmit_batch(
            0,
            [(packet(21, 8, 0), route_b), (packet(20, 8, 0), route_a)],
        )
        .unwrap();

    assert_eq!(transfers[0].packet().id(), FabricPacketId::new(20));
    assert_eq!(transfers[0].arrival_tick(), 4);
    assert_eq!(transfers[0].hops()[0].start_tick(), 2);
    assert_eq!(transfers[0].hops()[0].depart_tick(), 3);
    assert_eq!(
        transfers[0].hops()[0].router().unwrap().queue_delay_ticks(),
        0
    );
    assert_eq!(transfers[1].packet().id(), FabricPacketId::new(21));
    assert_eq!(transfers[1].arrival_tick(), 6);
    assert_eq!(transfers[1].hops()[0].start_tick(), 4);
    assert_eq!(transfers[1].hops()[0].depart_tick(), 5);
    let router_timing = transfers[1].hops()[0].router().unwrap();
    assert_eq!(router_timing.input_port(), 1);
    assert_eq!(router_timing.output_port(), 9);
    assert_eq!(router_timing.virtual_channel(), 1);
    assert_eq!(router_timing.queue_delay_ticks(), 2);

    let activities = fabric.hop_activities();
    assert_eq!(activities[0].router().unwrap().queue_delay_ticks(), 0);
    assert_eq!(activities[1].router().unwrap().queue_delay_ticks(), 2);
    assert_eq!(activities[0].queue_delay_ticks(), 0);
    assert_eq!(activities[1].queue_delay_ticks(), 0);
}

#[test]
fn failed_router_stage_transfer_does_not_consume_router_resources() {
    let mut fabric = FabricModel::new();
    let route = path([FabricPathHop::serial_link(
        link("router_overflow_serial"),
        ClockDomain::new(1).unwrap(),
        Cycles::new(1),
        1,
        1,
    )
    .unwrap()
    .with_router_stage(FabricRouterStage::new(router("router_overflow"), 0, 1, 0, 3).unwrap())]);
    let before = fabric.snapshot();

    assert_eq!(
        fabric
            .transmit(0, packet(99, u64::MAX, 0), route.clone())
            .unwrap_err(),
        FabricError::SerialLinkPacketBitOverflow { bytes: u64::MAX }
    );

    assert_eq!(fabric.snapshot(), before);
    assert!(fabric.hop_activities().is_empty());
    let transfer = fabric.transmit(0, packet(1, 1, 0), route).unwrap();
    let router = transfer.hops()[0].router().unwrap();
    assert_eq!(router.start_tick(), 0);
    assert_eq!(router.queue_delay_ticks(), 0);
}

#[test]
fn fabric_rejects_invalid_packets_paths_and_batches() {
    assert_eq!(FabricLinkId::new("").err(), Some(FabricError::EmptyLinkId));
    assert_eq!(
        FabricRouterId::new("").err(),
        Some(FabricError::EmptyRouterId)
    );
    assert_eq!(
        FabricPacket::new(FabricPacketId::new(1), 0, VirtualNetworkId::new(0)).err(),
        Some(FabricError::ZeroPacketBytes)
    );
    assert_eq!(
        FabricPathHop::new(link("mesh_x0"), 0, 8).err(),
        Some(FabricError::ZeroLinkLatency)
    );
    assert_eq!(
        FabricPathHop::new(link("mesh_x0"), 2, 0).err(),
        Some(FabricError::ZeroLinkBandwidth)
    );
    assert_eq!(
        FabricPathHop::new(link("mesh_x0"), 2, 8)
            .unwrap()
            .with_credit_depth(0)
            .err(),
        Some(FabricError::ZeroCreditDepth)
    );
    assert_eq!(
        FabricRouterStage::new(router("router0"), 0, 1, 0, 0).err(),
        Some(FabricError::ZeroRouterLatency)
    );
    assert_eq!(
        FabricPathHop::serial_link(
            link("serial_x0"),
            ClockDomain::new(1).unwrap(),
            Cycles::new(1),
            0,
            8,
        )
        .err(),
        Some(FabricError::ZeroSerialLinkLanes)
    );
    assert_eq!(
        FabricPathHop::serial_link(
            link("serial_x0"),
            ClockDomain::new(1).unwrap(),
            Cycles::new(1),
            4,
            0,
        )
        .err(),
        Some(FabricError::ZeroSerialLinkLaneSpeed)
    );
    assert_eq!(FabricPath::new([]).err(), Some(FabricError::EmptyPath));

    let mut fabric = FabricModel::new();
    let route = path([FabricPathHop::new(link("mesh_x0"), 2, 8).unwrap()]);
    assert_eq!(
        fabric
            .transmit_batch(
                0,
                [(packet(9, 8, 0), route.clone()), (packet(9, 8, 0), route),],
            )
            .err(),
        Some(FabricError::DuplicatePacketInBatch {
            packet: FabricPacketId::new(9),
        })
    );
}

#[test]
fn fabric_transaction_rolls_back_resource_and_activity_state() {
    let mut fabric = FabricModel::new();
    let good = path([FabricPathHop::new(link("mesh_transaction"), 2, 4).unwrap()]);
    let bad = path([FabricPathHop::new(link("mesh_transaction"), u64::MAX, 4).unwrap()]);
    let before = fabric.snapshot();

    let error = fabric
        .try_transaction(|fabric| -> Result<(), FabricError> {
            fabric.transmit(0, packet(1, 8, 0), good)?;
            fabric.transmit(0, packet(2, 8, 0), bad)?;
            Ok(())
        })
        .unwrap_err();

    assert_eq!(error, FabricError::TickOverflow);
    assert_eq!(fabric.snapshot(), before);
    assert!(fabric.hop_activities().is_empty());
    assert_eq!(fabric.total_transfer_count(), 0);
}

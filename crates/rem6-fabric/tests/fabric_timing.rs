use rem6_fabric::{
    FabricError, FabricLinkId, FabricModel, FabricPacket, FabricPacketId, FabricPath,
    FabricPathHop, VirtualNetworkId,
};
use rem6_kernel::{WaitForEdgeKind, WaitForNode};

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

fn path(hops: impl IntoIterator<Item = FabricPathHop>) -> FabricPath {
    FabricPath::new(hops).unwrap()
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
fn fabric_keeps_virtual_network_lanes_independent() {
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
    assert_eq!(transfers[1].arrival_tick(), 4);
    assert_eq!(transfers[1].hops()[0].start_tick(), 0);
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
    assert_eq!(transfers[2].hops()[0].ready_tick(), 11);
    assert_eq!(transfers[2].hops()[0].depart_tick(), 12);
    assert_eq!(transfers[2].arrival_tick(), 22);

    let activity = fabric
        .lane_activity(&link("mesh_credit"), VirtualNetworkId::new(1))
        .unwrap();
    assert_eq!(activity.transfer_count(), 3);
    assert_eq!(activity.byte_count(), 24);
    assert_eq!(activity.occupied_ticks(), 3);
    assert_eq!(activity.queue_delay_ticks(), 12);
    assert_eq!(activity.max_queue_delay_ticks(), 11);
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
    let credit = WaitForNode::resource("fabric.mesh_wait_credit.vn.1.credit").unwrap();
    let active_wait = fabric.wait_for_graph_at(2).snapshot();

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
    assert_eq!(transfers[1].hops()[0].start_tick(), 0);
    assert_eq!(transfers[1].arrival_tick(), 11);
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

    fabric.transmit(20, packet(9, 8, 1), route.clone()).unwrap();
    assert_ne!(fabric.lane_snapshots(), snapshot);

    fabric.restore_lane_snapshots(snapshot.clone()).unwrap();

    assert_eq!(fabric.lane_snapshots(), snapshot);
    let replayed = fabric.transmit(1, packet(3, 8, 1), route).unwrap();
    assert_eq!(replayed, expected_transfer);
    assert_eq!(fabric.lane_snapshots(), expected.lane_snapshots());
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
fn fabric_rejects_invalid_packets_paths_and_batches() {
    assert_eq!(FabricLinkId::new("").err(), Some(FabricError::EmptyLinkId));
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

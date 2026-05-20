use rem6_fabric::{
    FabricError, FabricLinkId, FabricModel, FabricPacket, FabricPacketId, FabricPath,
    FabricPathHop, VirtualNetworkId,
};

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

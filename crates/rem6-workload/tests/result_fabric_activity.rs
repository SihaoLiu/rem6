use rem6_fabric::{
    FabricLaneActivity, FabricLinkActivity, FabricLinkId, FabricVirtualNetworkActivity,
    VirtualNetworkId,
};
use rem6_workload::WorkloadParallelExecutionSummary;

fn link(value: &str) -> FabricLinkId {
    FabricLinkId::new(value).unwrap()
}

fn lane(
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
        VirtualNetworkId::new(virtual_network),
        transfer_count,
        byte_count,
        occupied_ticks,
        queue_delay_ticks,
        max_queue_delay_ticks,
        first_tick,
        last_tick,
    )
}

fn virtual_network(
    virtual_network: u16,
    active_lane_count: usize,
    transfer_count: usize,
    byte_count: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    contended_lane_count: usize,
    first_tick: u64,
    last_tick: u64,
) -> FabricVirtualNetworkActivity {
    FabricVirtualNetworkActivity::new(
        VirtualNetworkId::new(virtual_network),
        active_lane_count,
        transfer_count,
        byte_count,
        occupied_ticks,
        queue_delay_ticks,
        max_queue_delay_ticks,
        contended_lane_count,
        first_tick,
        last_tick,
    )
}

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
fn workload_result_preserves_fabric_lane_and_virtual_network_activity() {
    let summary = WorkloadParallelExecutionSummary::default().with_fabric_lane_activities([
        lane("mesh_a", 1, 2, 16, 2, 7, 5, 0, 9),
        lane("mesh_a", 1, 1, 8, 1, 3, 3, 12, 16),
        lane("mesh_b", 2, 1, 32, 4, 0, 0, 1, 5),
    ]);

    assert_eq!(summary.active_fabric_lane_count(), 2);
    assert_eq!(summary.fabric_transfer_count(), 4);
    assert_eq!(summary.fabric_byte_count(), 56);
    assert_eq!(summary.fabric_occupied_ticks(), 7);
    assert_eq!(summary.fabric_queue_delay_ticks(), 10);
    assert_eq!(summary.fabric_max_queue_delay_ticks(), 5);
    assert_eq!(summary.contended_fabric_lane_count(), 1);
    assert_eq!(
        summary
            .fabric_lane_activity(&link("mesh_a"), VirtualNetworkId::new(1))
            .unwrap(),
        lane("mesh_a", 1, 3, 24, 3, 10, 5, 0, 16),
    );

    let virtual_networks = summary.fabric_virtual_network_activities();
    assert_eq!(virtual_networks.len(), 2);
    assert_eq!(
        virtual_networks[0].virtual_network(),
        VirtualNetworkId::new(1)
    );
    assert_eq!(virtual_networks[0].transfer_count(), 3);
    assert_eq!(virtual_networks[0].queue_delay_ticks(), 10);
    assert_eq!(
        virtual_networks[1].virtual_network(),
        VirtualNetworkId::new(2)
    );
    assert_eq!(virtual_networks[1].byte_count(), 32);
    assert_eq!(
        summary
            .fabric_virtual_network_activity(VirtualNetworkId::new(1))
            .unwrap(),
        virtual_networks[0],
    );

    let links = summary.fabric_link_activities();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].link(), &link("mesh_a"));
    assert_eq!(links[0].active_virtual_network_count(), 1);
    assert_eq!(links[0].transfer_count(), 3);
    assert_eq!(links[0].queue_delay_ticks(), 10);
    assert_eq!(links[0].contended_virtual_network_count(), 1);
    assert_eq!(links[1].link(), &link("mesh_b"));
    assert_eq!(links[1].byte_count(), 32);
    assert_eq!(
        summary.fabric_link_activity(&link("mesh_a")).unwrap(),
        links[0],
    );

    let merged_virtual_networks = WorkloadParallelExecutionSummary::default()
        .with_fabric_virtual_network_activities([
            virtual_network(3, 1, 2, 64, 6, 4, 4, 1, 10, 20),
            virtual_network(3, 2, 3, 96, 8, 7, 5, 1, 2, 9),
        ]);
    assert_eq!(
        merged_virtual_networks.active_fabric_virtual_network_count(),
        1
    );
    assert_eq!(
        merged_virtual_networks
            .fabric_virtual_network_activity(VirtualNetworkId::new(3))
            .unwrap(),
        virtual_network(3, 3, 5, 160, 14, 11, 5, 2, 2, 20),
    );

    let merged_links = WorkloadParallelExecutionSummary::default().with_fabric_link_activities([
        link_activity("mesh_c", 1, 2, 64, 6, 4, 4, 1, 10, 20),
        link_activity("mesh_c", 2, 3, 96, 8, 7, 5, 1, 2, 9),
    ]);
    assert_eq!(merged_links.active_fabric_link_count(), 1);
    assert_eq!(
        merged_links.fabric_link_activity(&link("mesh_c")).unwrap(),
        link_activity("mesh_c", 3, 5, 160, 14, 11, 5, 2, 2, 20),
    );
}

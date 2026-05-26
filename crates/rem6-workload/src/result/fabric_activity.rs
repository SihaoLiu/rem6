use std::collections::BTreeMap;

use rem6_fabric::{
    FabricActivityProfile, FabricLaneActivity, FabricLinkActivity, FabricLinkId,
    FabricVirtualNetworkActivity, VirtualNetworkId,
};

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub fn with_fabric_lane_activities(
        mut self,
        activities: impl IntoIterator<Item = FabricLaneActivity>,
    ) -> Self {
        self.fabric_lane_activities = collect_fabric_lane_activities(activities);
        self.fabric_link_activities =
            FabricLinkActivity::from_lanes(self.fabric_lane_activities.iter());
        self.fabric_virtual_network_activities =
            FabricVirtualNetworkActivity::from_lanes(self.fabric_lane_activities.iter());
        let profile = FabricActivityProfile::from_lanes(self.fabric_lane_activities.iter());
        self.active_fabric_lane_count = profile.active_lane_count();
        self.fabric_transfer_count = profile.transfer_count();
        self.fabric_byte_count = profile.byte_count();
        self.fabric_occupied_ticks = profile.occupied_ticks();
        self.fabric_queue_delay_ticks = profile.queue_delay_ticks();
        self.fabric_max_queue_delay_ticks = profile.max_queue_delay_ticks();
        self.contended_fabric_lane_count = profile.contended_lane_count();
        self
    }

    pub fn with_fabric_link_activities(
        mut self,
        activities: impl IntoIterator<Item = FabricLinkActivity>,
    ) -> Self {
        self.fabric_link_activities = collect_fabric_link_activities(activities);
        self
    }

    pub fn with_fabric_virtual_network_activities(
        mut self,
        activities: impl IntoIterator<Item = FabricVirtualNetworkActivity>,
    ) -> Self {
        self.fabric_virtual_network_activities =
            collect_fabric_virtual_network_activities(activities);
        self
    }

    pub fn fabric_lane_activities(&self) -> &[FabricLaneActivity] {
        &self.fabric_lane_activities
    }

    pub fn fabric_link_activities(&self) -> &[FabricLinkActivity] {
        &self.fabric_link_activities
    }

    pub fn fabric_link_activity(&self, link: &FabricLinkId) -> Option<FabricLinkActivity> {
        self.fabric_link_activities
            .iter()
            .find(|activity| activity.link() == link)
            .cloned()
    }

    pub fn fabric_lane_activity(
        &self,
        link: &FabricLinkId,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricLaneActivity> {
        self.fabric_lane_activities
            .iter()
            .find(|activity| {
                activity.link() == link && activity.virtual_network() == virtual_network
            })
            .cloned()
    }

    pub fn fabric_virtual_network_activities(&self) -> &[FabricVirtualNetworkActivity] {
        &self.fabric_virtual_network_activities
    }

    pub fn fabric_virtual_network_activity(
        &self,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricVirtualNetworkActivity> {
        self.fabric_virtual_network_activities
            .iter()
            .find(|activity| activity.virtual_network() == virtual_network)
            .cloned()
    }

    pub fn active_fabric_virtual_network_count(&self) -> usize {
        self.fabric_virtual_network_activities
            .iter()
            .filter(|activity| !activity.is_empty())
            .count()
    }

    pub fn active_fabric_link_count(&self) -> usize {
        self.fabric_link_activities
            .iter()
            .filter(|activity| !activity.is_empty())
            .count()
    }
}

fn collect_fabric_lane_activities(
    activities: impl IntoIterator<Item = FabricLaneActivity>,
) -> Vec<FabricLaneActivity> {
    let mut by_lane = BTreeMap::<(FabricLinkId, VirtualNetworkId), FabricLaneActivity>::new();
    for activity in activities {
        if activity.transfer_count() == 0 {
            continue;
        }
        let key = (activity.link().clone(), activity.virtual_network());
        by_lane
            .entry(key)
            .and_modify(|stored| *stored = stored.clone().merge_window(activity.clone()))
            .or_insert(activity);
    }
    by_lane.into_values().collect()
}

fn collect_fabric_link_activities(
    activities: impl IntoIterator<Item = FabricLinkActivity>,
) -> Vec<FabricLinkActivity> {
    let mut by_link = BTreeMap::<FabricLinkId, FabricLinkActivity>::new();
    for activity in activities {
        if activity.is_empty() {
            continue;
        }
        by_link
            .entry(activity.link().clone())
            .and_modify(|stored| *stored = stored.clone().merge_window(activity.clone()))
            .or_insert(activity);
    }
    by_link.into_values().collect()
}

fn collect_fabric_virtual_network_activities(
    activities: impl IntoIterator<Item = FabricVirtualNetworkActivity>,
) -> Vec<FabricVirtualNetworkActivity> {
    let mut by_virtual_network = BTreeMap::<VirtualNetworkId, FabricVirtualNetworkActivity>::new();
    for activity in activities {
        if activity.is_empty() {
            continue;
        }
        by_virtual_network
            .entry(activity.virtual_network())
            .and_modify(|stored| *stored = stored.clone().merge_window(activity.clone()))
            .or_insert(activity);
    }
    by_virtual_network.into_values().collect()
}

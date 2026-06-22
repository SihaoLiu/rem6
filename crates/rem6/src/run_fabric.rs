use std::collections::BTreeSet;

use rem6_fabric::{FabricActivityProfile, FabricHopActivity, FabricLaneActivity, FabricModel};
use rem6_transport::MemoryTransport;

use crate::RunFabricConfig;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6RunFabricSummary {
    active_lanes: u64,
    active_virtual_networks: u64,
    transfers: u64,
    bytes: u64,
    flits: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
    max_credit_delay_ticks: u64,
    contended_lanes: u64,
    lane_activities: Vec<FabricLaneActivity>,
    hop_activities: Vec<FabricHopActivity>,
}

impl Rem6RunFabricSummary {
    pub(crate) fn from_transport(
        config: Option<&RunFabricConfig>,
        transport: &MemoryTransport,
    ) -> Self {
        if config.is_none() {
            return Self::default();
        }

        let lane_activities = transport.fabric_lane_activities().unwrap_or_default();
        let hop_activities = transport.fabric_hop_activities().unwrap_or_default();
        let profile = FabricActivityProfile::from_lanes(lane_activities.iter());
        let active_virtual_networks = lane_activities
            .iter()
            .map(FabricLaneActivity::virtual_network)
            .collect::<BTreeSet<_>>()
            .len() as u64;

        Self {
            active_lanes: profile.active_lane_count() as u64,
            active_virtual_networks,
            transfers: profile.transfer_count() as u64,
            bytes: profile.byte_count(),
            flits: profile.flit_count(),
            occupied_ticks: profile.occupied_ticks(),
            queue_delay_ticks: profile.queue_delay_ticks(),
            max_queue_delay_ticks: profile.max_queue_delay_ticks(),
            credit_delay_ticks: profile.credit_delay_ticks(),
            max_credit_delay_ticks: profile.max_credit_delay_ticks(),
            contended_lanes: profile.contended_lane_count() as u64,
            lane_activities,
            hop_activities,
        }
    }

    pub(crate) const fn active_lanes(&self) -> u64 {
        self.active_lanes
    }

    pub(crate) const fn active_virtual_networks(&self) -> u64 {
        self.active_virtual_networks
    }

    pub(crate) const fn transfers(&self) -> u64 {
        self.transfers
    }

    pub(crate) const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub(crate) const fn flits(&self) -> u64 {
        self.flits
    }

    pub(crate) const fn occupied_ticks(&self) -> u64 {
        self.occupied_ticks
    }

    pub(crate) const fn queue_delay_ticks(&self) -> u64 {
        self.queue_delay_ticks
    }

    pub(crate) const fn max_queue_delay_ticks(&self) -> u64 {
        self.max_queue_delay_ticks
    }

    pub(crate) const fn credit_delay_ticks(&self) -> u64 {
        self.credit_delay_ticks
    }

    pub(crate) const fn max_credit_delay_ticks(&self) -> u64 {
        self.max_credit_delay_ticks
    }

    pub(crate) const fn contended_lanes(&self) -> u64 {
        self.contended_lanes
    }

    pub(crate) fn lane_activities(&self) -> &[FabricLaneActivity] {
        &self.lane_activities
    }

    pub(crate) fn hop_activities(&self) -> &[FabricHopActivity] {
        &self.hop_activities
    }
}

pub(crate) fn run_memory_transport(config: Option<&RunFabricConfig>) -> MemoryTransport {
    match config {
        Some(_) => MemoryTransport::with_fabric(FabricModel::new()),
        None => MemoryTransport::new(),
    }
}

use std::collections::BTreeMap;

use rem6_kernel::Tick;

use crate::{FabricLinkId, VirtualNetworkId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricLaneActivity {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    transfer_count: usize,
    byte_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    max_queue_delay_ticks: Tick,
    first_tick: Tick,
    last_tick: Tick,
}

impl FabricLaneActivity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        transfer_count: usize,
        byte_count: u64,
        occupied_ticks: Tick,
        queue_delay_ticks: Tick,
        max_queue_delay_ticks: Tick,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Self {
        Self {
            link,
            virtual_network,
            transfer_count,
            byte_count,
            occupied_ticks,
            queue_delay_ticks,
            max_queue_delay_ticks,
            first_tick,
            last_tick,
        }
    }

    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }

    pub const fn transfer_count(&self) -> usize {
        self.transfer_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub const fn occupied_ticks(&self) -> Tick {
        self.occupied_ticks
    }

    pub const fn queue_delay_ticks(&self) -> Tick {
        self.queue_delay_ticks
    }

    pub const fn max_queue_delay_ticks(&self) -> Tick {
        self.max_queue_delay_ticks
    }

    pub const fn first_tick(&self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub const fn has_contention(&self) -> bool {
        self.queue_delay_ticks != 0
    }

    pub fn merge_window(self, later: Self) -> Self {
        debug_assert_eq!(&self.link, &later.link);
        debug_assert_eq!(self.virtual_network, later.virtual_network);
        Self {
            link: self.link,
            virtual_network: self.virtual_network,
            transfer_count: self.transfer_count + later.transfer_count,
            byte_count: self.byte_count + later.byte_count,
            occupied_ticks: self.occupied_ticks + later.occupied_ticks,
            queue_delay_ticks: self.queue_delay_ticks + later.queue_delay_ticks,
            max_queue_delay_ticks: self.max_queue_delay_ticks.max(later.max_queue_delay_ticks),
            first_tick: self.first_tick.min(later.first_tick),
            last_tick: self.last_tick.max(later.last_tick),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricLinkActivity {
    link: FabricLinkId,
    active_virtual_network_count: usize,
    transfer_count: usize,
    byte_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    max_queue_delay_ticks: Tick,
    contended_virtual_network_count: usize,
    first_tick: Tick,
    last_tick: Tick,
}

impl FabricLinkActivity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        link: FabricLinkId,
        active_virtual_network_count: usize,
        transfer_count: usize,
        byte_count: u64,
        occupied_ticks: Tick,
        queue_delay_ticks: Tick,
        max_queue_delay_ticks: Tick,
        contended_virtual_network_count: usize,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Self {
        let stored_first_tick = if first_tick <= last_tick {
            first_tick
        } else {
            last_tick
        };
        let stored_last_tick = if first_tick <= last_tick {
            last_tick
        } else {
            first_tick
        };
        Self {
            link,
            active_virtual_network_count,
            transfer_count,
            byte_count,
            occupied_ticks,
            queue_delay_ticks,
            max_queue_delay_ticks,
            contended_virtual_network_count,
            first_tick: stored_first_tick,
            last_tick: stored_last_tick,
        }
    }

    pub fn from_lanes<'a, I>(lanes: I) -> Vec<Self>
    where
        I: IntoIterator<Item = &'a FabricLaneActivity>,
    {
        let mut by_link = BTreeMap::<FabricLinkId, Self>::new();
        for lane in lanes {
            by_link
                .entry(lane.link().clone())
                .and_modify(|activity| activity.merge_lane(lane))
                .or_insert_with(|| Self::from_lane(lane));
        }
        by_link.into_values().collect()
    }

    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn active_virtual_network_count(&self) -> usize {
        self.active_virtual_network_count
    }

    pub const fn transfer_count(&self) -> usize {
        self.transfer_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub const fn occupied_ticks(&self) -> Tick {
        self.occupied_ticks
    }

    pub const fn queue_delay_ticks(&self) -> Tick {
        self.queue_delay_ticks
    }

    pub const fn max_queue_delay_ticks(&self) -> Tick {
        self.max_queue_delay_ticks
    }

    pub const fn contended_virtual_network_count(&self) -> usize {
        self.contended_virtual_network_count
    }

    pub const fn first_tick(&self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub const fn has_contention(&self) -> bool {
        self.contended_virtual_network_count != 0
    }

    pub const fn is_empty(&self) -> bool {
        self.transfer_count == 0
    }

    pub fn merge_window(self, later: Self) -> Self {
        debug_assert_eq!(&self.link, &later.link);
        Self {
            link: self.link,
            active_virtual_network_count: self.active_virtual_network_count
                + later.active_virtual_network_count,
            transfer_count: self.transfer_count + later.transfer_count,
            byte_count: self.byte_count + later.byte_count,
            occupied_ticks: self.occupied_ticks + later.occupied_ticks,
            queue_delay_ticks: self.queue_delay_ticks + later.queue_delay_ticks,
            max_queue_delay_ticks: self.max_queue_delay_ticks.max(later.max_queue_delay_ticks),
            contended_virtual_network_count: self.contended_virtual_network_count
                + later.contended_virtual_network_count,
            first_tick: self.first_tick.min(later.first_tick),
            last_tick: self.last_tick.max(later.last_tick),
        }
    }

    fn from_lane(lane: &FabricLaneActivity) -> Self {
        Self::new(
            lane.link().clone(),
            1,
            lane.transfer_count(),
            lane.byte_count(),
            lane.occupied_ticks(),
            lane.queue_delay_ticks(),
            lane.max_queue_delay_ticks(),
            usize::from(lane.has_contention()),
            lane.first_tick(),
            lane.last_tick(),
        )
    }

    fn merge_lane(&mut self, lane: &FabricLaneActivity) {
        debug_assert_eq!(&self.link, lane.link());
        self.active_virtual_network_count += 1;
        self.transfer_count += lane.transfer_count();
        self.byte_count += lane.byte_count();
        self.occupied_ticks += lane.occupied_ticks();
        self.queue_delay_ticks += lane.queue_delay_ticks();
        self.max_queue_delay_ticks = self.max_queue_delay_ticks.max(lane.max_queue_delay_ticks());
        if lane.has_contention() {
            self.contended_virtual_network_count += 1;
        }
        self.first_tick = self.first_tick.min(lane.first_tick());
        self.last_tick = self.last_tick.max(lane.last_tick());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricVirtualNetworkActivity {
    virtual_network: VirtualNetworkId,
    active_lane_count: usize,
    transfer_count: usize,
    byte_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    max_queue_delay_ticks: Tick,
    contended_lane_count: usize,
    first_tick: Tick,
    last_tick: Tick,
}

impl FabricVirtualNetworkActivity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        virtual_network: VirtualNetworkId,
        active_lane_count: usize,
        transfer_count: usize,
        byte_count: u64,
        occupied_ticks: Tick,
        queue_delay_ticks: Tick,
        max_queue_delay_ticks: Tick,
        contended_lane_count: usize,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Self {
        let stored_first_tick = if first_tick <= last_tick {
            first_tick
        } else {
            last_tick
        };
        let stored_last_tick = if first_tick <= last_tick {
            last_tick
        } else {
            first_tick
        };
        Self {
            virtual_network,
            active_lane_count,
            transfer_count,
            byte_count,
            occupied_ticks,
            queue_delay_ticks,
            max_queue_delay_ticks,
            contended_lane_count,
            first_tick: stored_first_tick,
            last_tick: stored_last_tick,
        }
    }

    pub fn from_lanes<'a, I>(lanes: I) -> Vec<Self>
    where
        I: IntoIterator<Item = &'a FabricLaneActivity>,
    {
        let mut by_virtual_network = BTreeMap::<VirtualNetworkId, Self>::new();
        for lane in lanes {
            by_virtual_network
                .entry(lane.virtual_network())
                .and_modify(|activity| activity.merge_lane(lane))
                .or_insert_with(|| Self::from_lane(lane));
        }
        by_virtual_network.into_values().collect()
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }

    pub const fn active_lane_count(&self) -> usize {
        self.active_lane_count
    }

    pub const fn transfer_count(&self) -> usize {
        self.transfer_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub const fn occupied_ticks(&self) -> Tick {
        self.occupied_ticks
    }

    pub const fn queue_delay_ticks(&self) -> Tick {
        self.queue_delay_ticks
    }

    pub const fn max_queue_delay_ticks(&self) -> Tick {
        self.max_queue_delay_ticks
    }

    pub const fn contended_lane_count(&self) -> usize {
        self.contended_lane_count
    }

    pub const fn first_tick(&self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub const fn has_contention(&self) -> bool {
        self.contended_lane_count != 0
    }

    pub const fn is_empty(&self) -> bool {
        self.transfer_count == 0
    }

    pub fn merge_window(self, later: Self) -> Self {
        debug_assert_eq!(self.virtual_network, later.virtual_network);
        Self {
            virtual_network: self.virtual_network,
            active_lane_count: self.active_lane_count + later.active_lane_count,
            transfer_count: self.transfer_count + later.transfer_count,
            byte_count: self.byte_count + later.byte_count,
            occupied_ticks: self.occupied_ticks + later.occupied_ticks,
            queue_delay_ticks: self.queue_delay_ticks + later.queue_delay_ticks,
            max_queue_delay_ticks: self.max_queue_delay_ticks.max(later.max_queue_delay_ticks),
            contended_lane_count: self.contended_lane_count + later.contended_lane_count,
            first_tick: self.first_tick.min(later.first_tick),
            last_tick: self.last_tick.max(later.last_tick),
        }
    }

    fn from_lane(lane: &FabricLaneActivity) -> Self {
        Self::new(
            lane.virtual_network(),
            1,
            lane.transfer_count(),
            lane.byte_count(),
            lane.occupied_ticks(),
            lane.queue_delay_ticks(),
            lane.max_queue_delay_ticks(),
            usize::from(lane.has_contention()),
            lane.first_tick(),
            lane.last_tick(),
        )
    }

    fn merge_lane(&mut self, lane: &FabricLaneActivity) {
        debug_assert_eq!(self.virtual_network, lane.virtual_network());
        self.active_lane_count += 1;
        self.transfer_count += lane.transfer_count();
        self.byte_count += lane.byte_count();
        self.occupied_ticks += lane.occupied_ticks();
        self.queue_delay_ticks += lane.queue_delay_ticks();
        self.max_queue_delay_ticks = self.max_queue_delay_ticks.max(lane.max_queue_delay_ticks());
        if lane.has_contention() {
            self.contended_lane_count += 1;
        }
        self.first_tick = self.first_tick.min(lane.first_tick());
        self.last_tick = self.last_tick.max(lane.last_tick());
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FabricActivityProfile {
    active_lane_count: usize,
    transfer_count: usize,
    byte_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    max_queue_delay_ticks: Tick,
    contended_lane_count: usize,
}

impl FabricActivityProfile {
    pub const fn new(
        active_lane_count: usize,
        transfer_count: usize,
        byte_count: u64,
        occupied_ticks: Tick,
        queue_delay_ticks: Tick,
        max_queue_delay_ticks: Tick,
        contended_lane_count: usize,
    ) -> Self {
        Self {
            active_lane_count,
            transfer_count,
            byte_count,
            occupied_ticks,
            queue_delay_ticks,
            max_queue_delay_ticks,
            contended_lane_count,
        }
    }

    pub fn from_lanes<'a, I>(lanes: I) -> Self
    where
        I: IntoIterator<Item = &'a FabricLaneActivity>,
    {
        let mut profile = Self::default();
        for lane in lanes {
            profile.active_lane_count += 1;
            profile.transfer_count += lane.transfer_count();
            profile.byte_count += lane.byte_count();
            profile.occupied_ticks += lane.occupied_ticks();
            profile.queue_delay_ticks += lane.queue_delay_ticks();
            profile.max_queue_delay_ticks = profile
                .max_queue_delay_ticks
                .max(lane.max_queue_delay_ticks());
            if lane.has_contention() {
                profile.contended_lane_count += 1;
            }
        }
        profile
    }

    pub const fn active_lane_count(self) -> usize {
        self.active_lane_count
    }

    pub const fn transfer_count(self) -> usize {
        self.transfer_count
    }

    pub const fn byte_count(self) -> u64 {
        self.byte_count
    }

    pub const fn occupied_ticks(self) -> Tick {
        self.occupied_ticks
    }

    pub const fn queue_delay_ticks(self) -> Tick {
        self.queue_delay_ticks
    }

    pub const fn max_queue_delay_ticks(self) -> Tick {
        self.max_queue_delay_ticks
    }

    pub const fn contended_lane_count(self) -> usize {
        self.contended_lane_count
    }

    pub const fn has_contention(self) -> bool {
        self.contended_lane_count != 0
    }

    pub const fn is_empty(self) -> bool {
        self.transfer_count == 0
    }
}

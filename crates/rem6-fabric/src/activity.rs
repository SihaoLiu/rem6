use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;

use crate::types::{FabricLinkId, VirtualNetworkId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricLaneActivity {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    transfer_count: usize,
    byte_count: u64,
    flit_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    max_queue_delay_ticks: Tick,
    credit_delay_ticks: Tick,
    max_credit_delay_ticks: Tick,
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
            flit_count: transfer_count as u64,
            occupied_ticks,
            queue_delay_ticks,
            max_queue_delay_ticks,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
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

    pub const fn flit_count(&self) -> u64 {
        self.flit_count
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

    pub const fn credit_delay_ticks(&self) -> Tick {
        self.credit_delay_ticks
    }

    pub const fn max_credit_delay_ticks(&self) -> Tick {
        self.max_credit_delay_ticks
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

    pub const fn with_flit_count(mut self, flit_count: u64) -> Self {
        self.flit_count = flit_count;
        self
    }

    pub const fn with_credit_delay(
        mut self,
        credit_delay_ticks: Tick,
        max_credit_delay_ticks: Tick,
    ) -> Self {
        self.credit_delay_ticks = credit_delay_ticks;
        self.max_credit_delay_ticks = max_credit_delay_ticks;
        self
    }

    pub fn merge_window(self, later: Self) -> Self {
        debug_assert_eq!(&self.link, &later.link);
        debug_assert_eq!(self.virtual_network, later.virtual_network);
        Self {
            link: self.link,
            virtual_network: self.virtual_network,
            transfer_count: self.transfer_count + later.transfer_count,
            byte_count: self.byte_count + later.byte_count,
            flit_count: self.flit_count + later.flit_count,
            occupied_ticks: self.occupied_ticks + later.occupied_ticks,
            queue_delay_ticks: self.queue_delay_ticks + later.queue_delay_ticks,
            max_queue_delay_ticks: self.max_queue_delay_ticks.max(later.max_queue_delay_ticks),
            credit_delay_ticks: self.credit_delay_ticks + later.credit_delay_ticks,
            max_credit_delay_ticks: self
                .max_credit_delay_ticks
                .max(later.max_credit_delay_ticks),
            first_tick: self.first_tick.min(later.first_tick),
            last_tick: self.last_tick.max(later.last_tick),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FabricLinkActivity {
    link: FabricLinkId,
    active_virtual_network_count: usize,
    active_virtual_networks: Option<BTreeSet<VirtualNetworkId>>,
    transfer_count: usize,
    byte_count: u64,
    flit_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    max_queue_delay_ticks: Tick,
    credit_delay_ticks: Tick,
    max_credit_delay_ticks: Tick,
    contended_virtual_network_count: usize,
    contended_virtual_networks: Option<BTreeSet<VirtualNetworkId>>,
    first_tick: Tick,
    last_tick: Tick,
}

impl PartialEq for FabricLinkActivity {
    fn eq(&self, other: &Self) -> bool {
        self.link == other.link
            && self.active_virtual_network_count == other.active_virtual_network_count
            && self.transfer_count == other.transfer_count
            && self.byte_count == other.byte_count
            && self.flit_count == other.flit_count
            && self.occupied_ticks == other.occupied_ticks
            && self.queue_delay_ticks == other.queue_delay_ticks
            && self.max_queue_delay_ticks == other.max_queue_delay_ticks
            && self.credit_delay_ticks == other.credit_delay_ticks
            && self.max_credit_delay_ticks == other.max_credit_delay_ticks
            && self.contended_virtual_network_count == other.contended_virtual_network_count
            && self.first_tick == other.first_tick
            && self.last_tick == other.last_tick
    }
}

impl Eq for FabricLinkActivity {}

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
            active_virtual_networks: None,
            transfer_count,
            byte_count,
            flit_count: transfer_count as u64,
            occupied_ticks,
            queue_delay_ticks,
            max_queue_delay_ticks,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
            contended_virtual_network_count,
            contended_virtual_networks: None,
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

    pub const fn flit_count(&self) -> u64 {
        self.flit_count
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

    pub const fn credit_delay_ticks(&self) -> Tick {
        self.credit_delay_ticks
    }

    pub const fn max_credit_delay_ticks(&self) -> Tick {
        self.max_credit_delay_ticks
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

    pub const fn with_flit_count(mut self, flit_count: u64) -> Self {
        self.flit_count = flit_count;
        self
    }

    pub const fn with_credit_delay(
        mut self,
        credit_delay_ticks: Tick,
        max_credit_delay_ticks: Tick,
    ) -> Self {
        self.credit_delay_ticks = credit_delay_ticks;
        self.max_credit_delay_ticks = max_credit_delay_ticks;
        self
    }

    pub const fn is_empty(&self) -> bool {
        self.transfer_count == 0
    }

    pub fn merge_window(self, later: Self) -> Self {
        debug_assert_eq!(&self.link, &later.link);
        let (active_virtual_network_count, active_virtual_networks) = merge_coverage(
            self.active_virtual_network_count,
            self.active_virtual_networks,
            later.active_virtual_network_count,
            later.active_virtual_networks,
        );
        let (contended_virtual_network_count, contended_virtual_networks) = merge_coverage(
            self.contended_virtual_network_count,
            self.contended_virtual_networks,
            later.contended_virtual_network_count,
            later.contended_virtual_networks,
        );
        Self {
            link: self.link,
            active_virtual_network_count,
            active_virtual_networks,
            transfer_count: self.transfer_count + later.transfer_count,
            byte_count: self.byte_count + later.byte_count,
            flit_count: self.flit_count + later.flit_count,
            occupied_ticks: self.occupied_ticks + later.occupied_ticks,
            queue_delay_ticks: self.queue_delay_ticks + later.queue_delay_ticks,
            max_queue_delay_ticks: self.max_queue_delay_ticks.max(later.max_queue_delay_ticks),
            credit_delay_ticks: self.credit_delay_ticks + later.credit_delay_ticks,
            max_credit_delay_ticks: self
                .max_credit_delay_ticks
                .max(later.max_credit_delay_ticks),
            contended_virtual_network_count,
            contended_virtual_networks,
            first_tick: self.first_tick.min(later.first_tick),
            last_tick: self.last_tick.max(later.last_tick),
        }
    }

    fn from_lane(lane: &FabricLaneActivity) -> Self {
        let active_virtual_networks = BTreeSet::from([lane.virtual_network()]);
        let mut contended_virtual_networks = BTreeSet::new();
        if lane.has_contention() {
            contended_virtual_networks.insert(lane.virtual_network());
        }
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
        .with_flit_count(lane.flit_count())
        .with_credit_delay(lane.credit_delay_ticks(), lane.max_credit_delay_ticks())
        .with_virtual_network_coverage(active_virtual_networks, Some(contended_virtual_networks))
    }

    fn merge_lane(&mut self, lane: &FabricLaneActivity) {
        debug_assert_eq!(&self.link, lane.link());
        if let Some(active_virtual_networks) = &mut self.active_virtual_networks {
            active_virtual_networks.insert(lane.virtual_network());
            self.active_virtual_network_count = active_virtual_networks.len();
        } else {
            self.active_virtual_network_count += 1;
        }
        self.transfer_count += lane.transfer_count();
        self.byte_count += lane.byte_count();
        self.flit_count += lane.flit_count();
        self.occupied_ticks += lane.occupied_ticks();
        self.queue_delay_ticks += lane.queue_delay_ticks();
        self.max_queue_delay_ticks = self.max_queue_delay_ticks.max(lane.max_queue_delay_ticks());
        self.credit_delay_ticks += lane.credit_delay_ticks();
        self.max_credit_delay_ticks = self
            .max_credit_delay_ticks
            .max(lane.max_credit_delay_ticks());
        if lane.has_contention() {
            if let Some(contended_virtual_networks) = &mut self.contended_virtual_networks {
                contended_virtual_networks.insert(lane.virtual_network());
                self.contended_virtual_network_count = contended_virtual_networks.len();
            } else {
                self.contended_virtual_network_count += 1;
            }
        }
        self.first_tick = self.first_tick.min(lane.first_tick());
        self.last_tick = self.last_tick.max(lane.last_tick());
    }

    fn with_virtual_network_coverage(
        mut self,
        active_virtual_networks: BTreeSet<VirtualNetworkId>,
        contended_virtual_networks: Option<BTreeSet<VirtualNetworkId>>,
    ) -> Self {
        self.active_virtual_network_count = active_virtual_networks.len();
        self.active_virtual_networks = Some(active_virtual_networks);
        self.contended_virtual_network_count =
            contended_virtual_networks.as_ref().map_or(0, BTreeSet::len);
        self.contended_virtual_networks = contended_virtual_networks;
        self
    }
}

#[derive(Clone, Debug)]
pub struct FabricVirtualNetworkActivity {
    virtual_network: VirtualNetworkId,
    active_lane_count: usize,
    active_links: Option<BTreeSet<FabricLinkId>>,
    transfer_count: usize,
    byte_count: u64,
    flit_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    max_queue_delay_ticks: Tick,
    credit_delay_ticks: Tick,
    max_credit_delay_ticks: Tick,
    contended_lane_count: usize,
    contended_links: Option<BTreeSet<FabricLinkId>>,
    first_tick: Tick,
    last_tick: Tick,
}

impl PartialEq for FabricVirtualNetworkActivity {
    fn eq(&self, other: &Self) -> bool {
        self.virtual_network == other.virtual_network
            && self.active_lane_count == other.active_lane_count
            && self.transfer_count == other.transfer_count
            && self.byte_count == other.byte_count
            && self.flit_count == other.flit_count
            && self.occupied_ticks == other.occupied_ticks
            && self.queue_delay_ticks == other.queue_delay_ticks
            && self.max_queue_delay_ticks == other.max_queue_delay_ticks
            && self.credit_delay_ticks == other.credit_delay_ticks
            && self.max_credit_delay_ticks == other.max_credit_delay_ticks
            && self.contended_lane_count == other.contended_lane_count
            && self.first_tick == other.first_tick
            && self.last_tick == other.last_tick
    }
}

impl Eq for FabricVirtualNetworkActivity {}

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
            active_links: None,
            transfer_count,
            byte_count,
            flit_count: transfer_count as u64,
            occupied_ticks,
            queue_delay_ticks,
            max_queue_delay_ticks,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
            contended_lane_count,
            contended_links: None,
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

    pub const fn flit_count(&self) -> u64 {
        self.flit_count
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

    pub const fn credit_delay_ticks(&self) -> Tick {
        self.credit_delay_ticks
    }

    pub const fn max_credit_delay_ticks(&self) -> Tick {
        self.max_credit_delay_ticks
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

    pub const fn with_flit_count(mut self, flit_count: u64) -> Self {
        self.flit_count = flit_count;
        self
    }

    pub const fn with_credit_delay(
        mut self,
        credit_delay_ticks: Tick,
        max_credit_delay_ticks: Tick,
    ) -> Self {
        self.credit_delay_ticks = credit_delay_ticks;
        self.max_credit_delay_ticks = max_credit_delay_ticks;
        self
    }

    pub const fn is_empty(&self) -> bool {
        self.transfer_count == 0
    }

    pub fn merge_window(self, later: Self) -> Self {
        debug_assert_eq!(self.virtual_network, later.virtual_network);
        let (active_lane_count, active_links) = merge_coverage(
            self.active_lane_count,
            self.active_links,
            later.active_lane_count,
            later.active_links,
        );
        let (contended_lane_count, contended_links) = merge_coverage(
            self.contended_lane_count,
            self.contended_links,
            later.contended_lane_count,
            later.contended_links,
        );
        Self {
            virtual_network: self.virtual_network,
            active_lane_count,
            active_links,
            transfer_count: self.transfer_count + later.transfer_count,
            byte_count: self.byte_count + later.byte_count,
            flit_count: self.flit_count + later.flit_count,
            occupied_ticks: self.occupied_ticks + later.occupied_ticks,
            queue_delay_ticks: self.queue_delay_ticks + later.queue_delay_ticks,
            max_queue_delay_ticks: self.max_queue_delay_ticks.max(later.max_queue_delay_ticks),
            credit_delay_ticks: self.credit_delay_ticks + later.credit_delay_ticks,
            max_credit_delay_ticks: self
                .max_credit_delay_ticks
                .max(later.max_credit_delay_ticks),
            contended_lane_count,
            contended_links,
            first_tick: self.first_tick.min(later.first_tick),
            last_tick: self.last_tick.max(later.last_tick),
        }
    }

    fn from_lane(lane: &FabricLaneActivity) -> Self {
        let active_links = BTreeSet::from([lane.link().clone()]);
        let mut contended_links = BTreeSet::new();
        if lane.has_contention() {
            contended_links.insert(lane.link().clone());
        }
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
        .with_flit_count(lane.flit_count())
        .with_credit_delay(lane.credit_delay_ticks(), lane.max_credit_delay_ticks())
        .with_link_coverage(active_links, Some(contended_links))
    }

    fn merge_lane(&mut self, lane: &FabricLaneActivity) {
        debug_assert_eq!(self.virtual_network, lane.virtual_network());
        if let Some(active_links) = &mut self.active_links {
            active_links.insert(lane.link().clone());
            self.active_lane_count = active_links.len();
        } else {
            self.active_lane_count += 1;
        }
        self.transfer_count += lane.transfer_count();
        self.byte_count += lane.byte_count();
        self.flit_count += lane.flit_count();
        self.occupied_ticks += lane.occupied_ticks();
        self.queue_delay_ticks += lane.queue_delay_ticks();
        self.max_queue_delay_ticks = self.max_queue_delay_ticks.max(lane.max_queue_delay_ticks());
        self.credit_delay_ticks += lane.credit_delay_ticks();
        self.max_credit_delay_ticks = self
            .max_credit_delay_ticks
            .max(lane.max_credit_delay_ticks());
        if lane.has_contention() {
            if let Some(contended_links) = &mut self.contended_links {
                contended_links.insert(lane.link().clone());
                self.contended_lane_count = contended_links.len();
            } else {
                self.contended_lane_count += 1;
            }
        }
        self.first_tick = self.first_tick.min(lane.first_tick());
        self.last_tick = self.last_tick.max(lane.last_tick());
    }

    fn with_link_coverage(
        mut self,
        active_links: BTreeSet<FabricLinkId>,
        contended_links: Option<BTreeSet<FabricLinkId>>,
    ) -> Self {
        self.active_lane_count = active_links.len();
        self.active_links = Some(active_links);
        self.contended_lane_count = contended_links.as_ref().map_or(0, BTreeSet::len);
        self.contended_links = contended_links;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FabricActivityProfile {
    active_lane_count: usize,
    transfer_count: usize,
    byte_count: u64,
    flit_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    max_queue_delay_ticks: Tick,
    credit_delay_ticks: Tick,
    max_credit_delay_ticks: Tick,
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
            flit_count: transfer_count as u64,
            occupied_ticks,
            queue_delay_ticks,
            max_queue_delay_ticks,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
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
            profile.flit_count += lane.flit_count();
            profile.occupied_ticks += lane.occupied_ticks();
            profile.queue_delay_ticks += lane.queue_delay_ticks();
            profile.max_queue_delay_ticks = profile
                .max_queue_delay_ticks
                .max(lane.max_queue_delay_ticks());
            profile.credit_delay_ticks += lane.credit_delay_ticks();
            profile.max_credit_delay_ticks = profile
                .max_credit_delay_ticks
                .max(lane.max_credit_delay_ticks());
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

    pub const fn flit_count(self) -> u64 {
        self.flit_count
    }

    pub const fn with_flit_count(mut self, flit_count: u64) -> Self {
        self.flit_count = flit_count;
        self
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

    pub const fn credit_delay_ticks(self) -> Tick {
        self.credit_delay_ticks
    }

    pub const fn max_credit_delay_ticks(self) -> Tick {
        self.max_credit_delay_ticks
    }

    pub const fn with_credit_delay(
        mut self,
        credit_delay_ticks: Tick,
        max_credit_delay_ticks: Tick,
    ) -> Self {
        self.credit_delay_ticks = credit_delay_ticks;
        self.max_credit_delay_ticks = max_credit_delay_ticks;
        self
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

fn merge_coverage<T: Ord>(
    left_count: usize,
    left: Option<BTreeSet<T>>,
    right_count: usize,
    right: Option<BTreeSet<T>>,
) -> (usize, Option<BTreeSet<T>>) {
    match (left, right) {
        (Some(mut left), Some(right)) => {
            left.extend(right);
            (left.len(), Some(left))
        }
        _ => (left_count + right_count, None),
    }
}

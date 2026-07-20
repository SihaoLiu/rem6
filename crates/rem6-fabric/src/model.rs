use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{Tick, WaitForEdgeKind, WaitForGraph, WaitForNode};

use crate::activity::{
    FabricActivityProfile, FabricLaneActivity, FabricLinkActivity, FabricVirtualNetworkActivity,
};
use crate::path::{FabricPath, FabricRouterStage};
use crate::qos::{FabricQosRequest, QosQueueArbiter};
use crate::snapshot::{
    FabricLaneSnapshot, FabricRouterInputVcSnapshot, FabricRouterOutputPortSnapshot, FabricSnapshot,
};
use crate::telemetry::{
    FabricActivityMarker, FabricHopActivity, FabricHopTiming, FabricRouterTiming, FabricTransfer,
    FabricWaitForMarker,
};
use crate::types::{
    FabricError, FabricLinkId, FabricPacket, FabricPacketId, FabricRouterId, VirtualNetworkId,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FabricLaneKey {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
}

impl FabricLaneKey {
    fn new(link: FabricLinkId, virtual_network: VirtualNetworkId) -> Self {
        Self {
            link,
            virtual_network,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FabricRouterInputVcKey {
    router: FabricRouterId,
    input_port: u32,
    virtual_channel: u16,
}

impl FabricRouterInputVcKey {
    fn new(router: FabricRouterId, input_port: u32, virtual_channel: u16) -> Self {
        Self {
            router,
            input_port,
            virtual_channel,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FabricRouterOutputPortKey {
    router: FabricRouterId,
    output_port: u32,
}

impl FabricRouterOutputPortKey {
    fn new(router: FabricRouterId, output_port: u32) -> Self {
        Self {
            router,
            output_port,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FabricWaitKind {
    Queue,
    Link,
    Credit,
}

impl FabricWaitKind {
    const fn edge_kind(self) -> WaitForEdgeKind {
        match self {
            Self::Queue | Self::Link => WaitForEdgeKind::Queue,
            Self::Credit => WaitForEdgeKind::Credit,
        }
    }

    const fn resource_suffix(self) -> &'static str {
        match self {
            Self::Queue => "lane",
            Self::Link => "link",
            Self::Credit => "credit",
        }
    }

    const fn is_virtual_network_scoped(self) -> bool {
        !matches!(self, Self::Link)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FabricWaitRecord {
    packet: FabricPacketId,
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    kind: FabricWaitKind,
    first_blocked_tick: Tick,
    unblocked_tick: Tick,
}

impl FabricWaitRecord {
    fn new(
        packet: FabricPacketId,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        kind: FabricWaitKind,
        first_blocked_tick: Tick,
        unblocked_tick: Tick,
    ) -> Self {
        Self {
            packet,
            link,
            virtual_network,
            kind,
            first_blocked_tick,
            unblocked_tick,
        }
    }

    const fn is_active_at(&self, tick: Tick) -> bool {
        self.first_blocked_tick <= tick && tick < self.unblocked_tick
    }
}

#[derive(Clone, Debug, Default)]
struct FabricLaneState {
    next_available: Tick,
    credit_returns: Vec<Tick>,
}

impl FabricLaneState {
    fn from_snapshot(snapshot: &FabricLaneSnapshot) -> Self {
        let mut credit_returns = snapshot.credit_return_ticks().to_vec();
        credit_returns.sort_unstable();
        Self {
            next_available: snapshot.next_available_tick(),
            credit_returns,
        }
    }

    fn reserve(
        &mut self,
        arrival_tick: Tick,
        serialization_ticks: Tick,
        link_latency: Tick,
        credit_depth: Option<u32>,
        link_next_available: Tick,
    ) -> Result<FabricLaneReservation, FabricError> {
        let mut start_tick = arrival_tick
            .max(self.next_available)
            .max(link_next_available);
        let queue_wait_kind =
            queue_wait_kind(arrival_tick, self.next_available, link_next_available);
        let mut credit_delay_ticks: Tick = 0;
        if let Some(credit_depth) = credit_depth {
            loop {
                self.credit_returns
                    .retain(|credit_return| *credit_return > start_tick);
                if self.credit_returns.len() < credit_depth as usize {
                    break;
                }

                let credit_ready_tick = self.credit_returns[0];
                if credit_ready_tick <= start_tick {
                    break;
                }
                let previous_start_tick = start_tick;
                start_tick = credit_ready_tick
                    .max(self.next_available)
                    .max(link_next_available);
                let credit_delay = start_tick
                    .checked_sub(previous_start_tick)
                    .ok_or(FabricError::TickOverflow)?;
                credit_delay_ticks = credit_delay_ticks
                    .checked_add(credit_delay)
                    .ok_or(FabricError::TickOverflow)?;
            }
        }

        let depart_tick = start_tick
            .checked_add(serialization_ticks)
            .ok_or(FabricError::TickOverflow)?;
        let next_arrival_tick = depart_tick
            .checked_add(link_latency)
            .ok_or(FabricError::TickOverflow)?;

        self.next_available = depart_tick;
        if credit_depth.is_some() {
            self.credit_returns.push(next_arrival_tick);
            self.credit_returns.sort_unstable();
        }

        Ok(FabricLaneReservation {
            start_tick,
            depart_tick,
            arrival_tick: next_arrival_tick,
            queue_wait_kind,
            credit_delay_ticks,
        })
    }
}

fn lane_states_from_snapshots<I>(snapshots: I) -> Result<FabricLaneRestoreState, FabricError>
where
    I: IntoIterator<Item = FabricLaneSnapshot>,
{
    let mut lanes = BTreeMap::new();
    let mut links = BTreeMap::<FabricLinkId, FabricLinkState>::new();
    for snapshot in snapshots {
        let link = snapshot.link().clone();
        let virtual_network = snapshot.virtual_network();
        let next_available_tick = snapshot.next_available_tick();
        let key = FabricLaneKey::new(link.clone(), virtual_network);
        if lanes.contains_key(&key) {
            return Err(FabricError::DuplicateLaneSnapshot {
                link,
                virtual_network,
            });
        }
        links
            .entry(link)
            .and_modify(|state| {
                state.next_available = state.next_available.max(next_available_tick);
            })
            .or_insert(FabricLinkState {
                next_available: next_available_tick,
            });
        lanes.insert(key, FabricLaneState::from_snapshot(&snapshot));
    }
    Ok(FabricLaneRestoreState { lanes, links })
}

fn queue_wait_kind(
    arrival_tick: Tick,
    lane_next_available: Tick,
    link_next_available: Tick,
) -> Option<FabricWaitKind> {
    let start_tick = arrival_tick
        .max(lane_next_available)
        .max(link_next_available);
    if start_tick == arrival_tick {
        return None;
    }
    if link_next_available > lane_next_available && link_next_available > arrival_tick {
        Some(FabricWaitKind::Link)
    } else {
        Some(FabricWaitKind::Queue)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FabricLaneReservation {
    start_tick: Tick,
    depart_tick: Tick,
    arrival_tick: Tick,
    queue_wait_kind: Option<FabricWaitKind>,
    credit_delay_ticks: Tick,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FabricLinkState {
    next_available: Tick,
}

struct FabricLaneRestoreState {
    lanes: BTreeMap<FabricLaneKey, FabricLaneState>,
    links: BTreeMap<FabricLinkId, FabricLinkState>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FabricRouterResourceState {
    next_available: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FabricRouterReservation {
    input_key: FabricRouterInputVcKey,
    output_key: FabricRouterOutputPortKey,
    timing: FabricRouterTiming,
}

fn router_input_vc_states_from_snapshots<I>(
    snapshots: I,
) -> Result<BTreeMap<FabricRouterInputVcKey, FabricRouterResourceState>, FabricError>
where
    I: IntoIterator<Item = FabricRouterInputVcSnapshot>,
{
    let mut resources = BTreeMap::new();
    for snapshot in snapshots {
        let router = snapshot.router().clone();
        let input_port = snapshot.input_port();
        let virtual_channel = snapshot.virtual_channel();
        let key = FabricRouterInputVcKey::new(router.clone(), input_port, virtual_channel);
        if resources.contains_key(&key) {
            return Err(FabricError::DuplicateRouterInputVcSnapshot {
                router,
                input_port,
                virtual_channel,
            });
        }
        resources.insert(
            key,
            FabricRouterResourceState {
                next_available: snapshot.next_available_tick(),
            },
        );
    }
    Ok(resources)
}

fn router_output_port_states_from_snapshots<I>(
    snapshots: I,
) -> Result<BTreeMap<FabricRouterOutputPortKey, FabricRouterResourceState>, FabricError>
where
    I: IntoIterator<Item = FabricRouterOutputPortSnapshot>,
{
    let mut resources = BTreeMap::new();
    for snapshot in snapshots {
        let router = snapshot.router().clone();
        let output_port = snapshot.output_port();
        let key = FabricRouterOutputPortKey::new(router.clone(), output_port);
        if resources.contains_key(&key) {
            return Err(FabricError::DuplicateRouterOutputPortSnapshot {
                router,
                output_port,
            });
        }
        resources.insert(
            key,
            FabricRouterResourceState {
                next_available: snapshot.next_available_tick(),
            },
        );
    }
    Ok(resources)
}

#[derive(Clone, Debug, Default)]
pub struct FabricModel {
    links: BTreeMap<FabricLinkId, FabricLinkState>,
    lanes: BTreeMap<FabricLaneKey, FabricLaneState>,
    router_input_vcs: BTreeMap<FabricRouterInputVcKey, FabricRouterResourceState>,
    router_output_ports: BTreeMap<FabricRouterOutputPortKey, FabricRouterResourceState>,
    activity_log: Vec<FabricHopActivity>,
    wait_log: Vec<FabricWaitRecord>,
}

pub struct FabricTransaction<'a> {
    model: &'a mut FabricModel,
}

impl FabricTransaction<'_> {
    pub fn transmit(
        &mut self,
        injection_tick: Tick,
        packet: FabricPacket,
        path: FabricPath,
    ) -> Result<FabricTransfer, FabricError> {
        self.model.transmit(injection_tick, packet, path)
    }

    pub fn transmit_batch<I>(
        &mut self,
        injection_tick: Tick,
        requests: I,
    ) -> Result<Vec<FabricTransfer>, FabricError>
    where
        I: IntoIterator<Item = (FabricPacket, FabricPath)>,
    {
        self.model.transmit_batch(injection_tick, requests)
    }
}

impl FabricModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_transaction<T, E, F>(&mut self, transaction: F) -> Result<T, E>
    where
        F: FnOnce(&mut FabricTransaction<'_>) -> Result<T, E>,
    {
        let snapshot = self.snapshot();
        let activity_len = self.activity_log.len();
        let wait_len = self.wait_log.len();
        let result = transaction(&mut FabricTransaction { model: self });
        if result.is_err() {
            self.restore_snapshot(snapshot)
                .expect("a freshly captured fabric snapshot must restore");
            self.activity_log.truncate(activity_len);
            self.wait_log.truncate(wait_len);
        }
        result
    }

    pub fn transmit(
        &mut self,
        injection_tick: Tick,
        packet: FabricPacket,
        path: FabricPath,
    ) -> Result<FabricTransfer, FabricError> {
        self.reserve_transfer(injection_tick, packet, &path)
    }

    pub fn transmit_batch<I>(
        &mut self,
        injection_tick: Tick,
        requests: I,
    ) -> Result<Vec<FabricTransfer>, FabricError>
    where
        I: IntoIterator<Item = (FabricPacket, FabricPath)>,
    {
        let mut requests: Vec<_> = requests.into_iter().collect();
        let mut seen = BTreeSet::new();
        for (packet, _) in &requests {
            if !seen.insert(packet.id()) {
                return Err(FabricError::DuplicatePacketInBatch {
                    packet: packet.id(),
                });
            }
        }

        requests.sort_by_key(|(packet, _)| packet.id());
        requests
            .into_iter()
            .map(|(packet, path)| self.reserve_transfer(injection_tick, packet, &path))
            .collect()
    }

    pub fn transmit_qos_batch<I>(
        &mut self,
        injection_tick: Tick,
        requests: I,
        arbiter: &mut QosQueueArbiter,
    ) -> Result<Vec<FabricTransfer>, FabricError>
    where
        I: IntoIterator<Item = FabricQosRequest>,
    {
        let mut pending: Vec<_> = requests.into_iter().collect();
        let mut seen = BTreeSet::new();
        for request in &pending {
            if !seen.insert(request.packet().id()) {
                return Err(FabricError::DuplicatePacketInBatch {
                    packet: request.packet().id(),
                });
            }
        }

        let mut transfers = Vec::with_capacity(pending.len());
        while !pending.is_empty() {
            let queue: Vec<_> = pending
                .iter()
                .map(FabricQosRequest::queued_request)
                .collect();
            let Some(grant) = arbiter.grant(&queue) else {
                return Err(FabricError::QosNoGrant);
            };
            let request = pending.remove(grant.queue_index());
            let (packet, path) = request.into_packet_path();
            transfers.push(self.reserve_transfer(injection_tick, packet, &path)?);
        }

        Ok(transfers)
    }

    pub fn lane_snapshots(&self) -> Vec<FabricLaneSnapshot> {
        self.lanes
            .iter()
            .map(|(lane, state)| {
                FabricLaneSnapshot::new(
                    lane.link.clone(),
                    lane.virtual_network,
                    state.next_available,
                    state.credit_returns.clone(),
                )
            })
            .collect()
    }

    pub fn snapshot(&self) -> FabricSnapshot {
        let router_input_vcs = self
            .router_input_vcs
            .iter()
            .map(|(key, state)| {
                FabricRouterInputVcSnapshot::new(
                    key.router.clone(),
                    key.input_port,
                    key.virtual_channel,
                    state.next_available,
                )
            })
            .collect();
        let router_output_ports = self
            .router_output_ports
            .iter()
            .map(|(key, state)| {
                FabricRouterOutputPortSnapshot::new(
                    key.router.clone(),
                    key.output_port,
                    state.next_available,
                )
            })
            .collect();

        FabricSnapshot::new(self.lane_snapshots(), router_input_vcs, router_output_ports)
    }

    pub fn restore_lane_snapshots<I>(&mut self, snapshots: I) -> Result<(), FabricError>
    where
        I: IntoIterator<Item = FabricLaneSnapshot>,
    {
        let state = lane_states_from_snapshots(snapshots)?;
        self.lanes = state.lanes;
        self.links = state.links;
        self.router_input_vcs.clear();
        self.router_output_ports.clear();
        Ok(())
    }

    pub fn restore_snapshot(&mut self, snapshot: FabricSnapshot) -> Result<(), FabricError> {
        let (lane_snapshots, router_input_vc_snapshots, router_output_port_snapshots) =
            snapshot.into_parts();
        let state = lane_states_from_snapshots(lane_snapshots)?;
        let router_input_vcs = router_input_vc_states_from_snapshots(router_input_vc_snapshots)?;
        let router_output_ports =
            router_output_port_states_from_snapshots(router_output_port_snapshots)?;

        self.lanes = state.lanes;
        self.links = state.links;
        self.router_input_vcs = router_input_vcs;
        self.router_output_ports = router_output_ports;
        Ok(())
    }

    pub fn mark_activity(&self) -> FabricActivityMarker {
        FabricActivityMarker::new(self.activity_log.len())
    }

    pub fn mark_wait_for(&self) -> FabricWaitForMarker {
        FabricWaitForMarker::new(self.wait_log.len())
    }

    pub fn lane_activities(&self) -> Vec<FabricLaneActivity> {
        collect_lane_activities(&self.activity_log)
    }

    pub fn lane_activities_since(&self, marker: FabricActivityMarker) -> Vec<FabricLaneActivity> {
        let Some(records) = self.activity_log.get(marker.offset()..) else {
            return Vec::new();
        };
        collect_lane_activities(records)
    }

    pub fn hop_activities(&self) -> Vec<FabricHopActivity> {
        self.activity_log.clone()
    }

    pub fn hop_activities_since(&self, marker: FabricActivityMarker) -> Vec<FabricHopActivity> {
        let Some(activities) = self.activity_log.get(marker.offset()..) else {
            return Vec::new();
        };
        activities.to_vec()
    }

    pub fn link_activities(&self) -> Vec<FabricLinkActivity> {
        FabricLinkActivity::from_lanes(self.lane_activities().iter())
    }

    pub fn link_activities_since(&self, marker: FabricActivityMarker) -> Vec<FabricLinkActivity> {
        FabricLinkActivity::from_lanes(self.lane_activities_since(marker).iter())
    }

    pub fn virtual_network_activities(&self) -> Vec<FabricVirtualNetworkActivity> {
        FabricVirtualNetworkActivity::from_lanes(self.lane_activities().iter())
    }

    pub fn virtual_network_activities_since(
        &self,
        marker: FabricActivityMarker,
    ) -> Vec<FabricVirtualNetworkActivity> {
        FabricVirtualNetworkActivity::from_lanes(self.lane_activities_since(marker).iter())
    }

    pub fn activity_profile(&self) -> FabricActivityProfile {
        FabricActivityProfile::from_lanes(self.lane_activities().iter())
    }

    pub fn activity_profile_since(&self, marker: FabricActivityMarker) -> FabricActivityProfile {
        FabricActivityProfile::from_lanes(self.lane_activities_since(marker).iter())
    }

    pub fn lane_activity(
        &self,
        link: &FabricLinkId,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricLaneActivity> {
        self.lane_activities().into_iter().find(|activity| {
            activity.link() == link && activity.virtual_network() == virtual_network
        })
    }

    pub fn link_activity(&self, link: &FabricLinkId) -> Option<FabricLinkActivity> {
        self.link_activities()
            .into_iter()
            .find(|activity| activity.link() == link)
    }

    pub fn virtual_network_activity(
        &self,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricVirtualNetworkActivity> {
        self.virtual_network_activities()
            .into_iter()
            .find(|activity| activity.virtual_network() == virtual_network)
    }

    pub fn active_lane_count(&self) -> usize {
        self.lane_activities().len()
    }

    pub fn total_transfer_count(&self) -> usize {
        self.lane_activities()
            .iter()
            .map(FabricLaneActivity::transfer_count)
            .sum()
    }

    pub fn total_queue_delay_ticks(&self) -> Tick {
        self.lane_activities()
            .iter()
            .map(FabricLaneActivity::queue_delay_ticks)
            .sum()
    }

    pub fn wait_for_graph_at(&self, tick: Tick) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        for wait in self.wait_log.iter().filter(|wait| wait.is_active_at(tick)) {
            record_wait_interval(&mut graph, wait, tick, tick);
        }
        graph
    }

    pub fn wait_for_graph_since(&self, marker: FabricWaitForMarker) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        let Some(records) = self.wait_log.get(marker.offset()..) else {
            return graph;
        };
        for wait in records {
            record_wait_interval(
                &mut graph,
                wait,
                wait.first_blocked_tick,
                wait.unblocked_tick.saturating_sub(1),
            );
        }
        graph
    }

    pub fn clear_activity(&mut self) {
        self.activity_log.clear();
        self.wait_log.clear();
    }

    fn reserve_router_stage(
        &self,
        stage: &FabricRouterStage,
        ready_tick: Tick,
    ) -> Result<FabricRouterReservation, FabricError> {
        let input_key = FabricRouterInputVcKey::new(
            stage.router().clone(),
            stage.input_port(),
            stage.virtual_channel(),
        );
        let output_key =
            FabricRouterOutputPortKey::new(stage.router().clone(), stage.output_port());
        let input_ready_tick = self
            .router_input_vcs
            .get(&input_key)
            .map_or(0, |state| state.next_available);
        let output_ready_tick = self
            .router_output_ports
            .get(&output_key)
            .map_or(0, |state| state.next_available);
        let start_tick = ready_tick.max(input_ready_tick).max(output_ready_tick);
        let depart_tick = start_tick
            .checked_add(stage.latency())
            .ok_or(FabricError::TickOverflow)?;
        let queue_delay_ticks = start_tick
            .checked_sub(ready_tick)
            .ok_or(FabricError::TickOverflow)?;

        Ok(FabricRouterReservation {
            input_key,
            output_key,
            timing: FabricRouterTiming::new(
                stage.router().clone(),
                stage.input_port(),
                stage.output_port(),
                stage.virtual_channel(),
                ready_tick,
                start_tick,
                stage.latency(),
                depart_tick,
                queue_delay_ticks,
            ),
        })
    }

    fn commit_router_reservation(
        &mut self,
        reservation: FabricRouterReservation,
    ) -> FabricRouterTiming {
        let depart_tick = reservation.timing.depart_tick();
        self.router_input_vcs
            .entry(reservation.input_key)
            .or_default()
            .next_available = depart_tick;
        self.router_output_ports
            .entry(reservation.output_key)
            .or_default()
            .next_available = depart_tick;
        reservation.timing
    }

    fn reserve_transfer(
        &mut self,
        injection_tick: Tick,
        packet: FabricPacket,
        path: &FabricPath,
    ) -> Result<FabricTransfer, FabricError> {
        let mut arrival_tick = injection_tick;
        let mut timings = Vec::with_capacity(path.hops().len());

        for (hop_index, hop) in path.hops().iter().enumerate() {
            let ingress_tick = arrival_tick;
            let router_reservation = if let Some(stage) = hop.router_stage() {
                Some(self.reserve_router_stage(stage, ingress_tick)?)
            } else {
                None
            };
            let lane_ready_tick = router_reservation
                .as_ref()
                .map_or(ingress_tick, |reservation| reservation.timing.depart_tick());
            let virtual_network = hop.virtual_network().unwrap_or(packet.virtual_network());
            let lane = FabricLaneKey::new(hop.link().clone(), virtual_network);
            let serialization_ticks = hop.serialization_ticks(packet.bytes())?;
            let flits = hop.flit_count(packet.bytes())?;
            let link_next_available = self
                .links
                .get(hop.link())
                .map_or(0, |state| state.next_available);
            let reservation = self.lanes.entry(lane).or_default().reserve(
                lane_ready_tick,
                serialization_ticks,
                hop.latency(),
                hop.credit_depth(),
                link_next_available,
            )?;
            let router_timing =
                router_reservation.map(|reservation| self.commit_router_reservation(reservation));
            self.links
                .entry(hop.link().clone())
                .or_default()
                .next_available = reservation.depart_tick;
            let queue_delay_ticks = reservation
                .start_tick
                .checked_sub(lane_ready_tick)
                .ok_or(FabricError::TickOverflow)?;
            let credit_delay_ticks = reservation.credit_delay_ticks;
            if queue_delay_ticks != 0 {
                if credit_delay_ticks == 0 {
                    let wait_kind = reservation.queue_wait_kind.unwrap_or(FabricWaitKind::Queue);
                    self.wait_log.push(FabricWaitRecord::new(
                        packet.id(),
                        hop.link().clone(),
                        virtual_network,
                        wait_kind,
                        lane_ready_tick,
                        reservation.start_tick,
                    ));
                } else {
                    let credit_wait_start_tick = reservation
                        .start_tick
                        .checked_sub(credit_delay_ticks)
                        .ok_or(FabricError::TickOverflow)?;
                    if credit_wait_start_tick > lane_ready_tick {
                        let wait_kind =
                            reservation.queue_wait_kind.unwrap_or(FabricWaitKind::Queue);
                        self.wait_log.push(FabricWaitRecord::new(
                            packet.id(),
                            hop.link().clone(),
                            virtual_network,
                            wait_kind,
                            lane_ready_tick,
                            credit_wait_start_tick,
                        ));
                    }
                    self.wait_log.push(FabricWaitRecord::new(
                        packet.id(),
                        hop.link().clone(),
                        virtual_network,
                        FabricWaitKind::Credit,
                        credit_wait_start_tick,
                        reservation.start_tick,
                    ));
                }
            }

            let timing = FabricHopTiming::new(
                hop.link().clone(),
                virtual_network,
                router_timing,
                ingress_tick,
                reservation.start_tick,
                serialization_ticks,
                reservation.depart_tick,
                reservation.arrival_tick,
            );
            let activity = FabricHopActivity::new(
                packet.id(),
                hop_index,
                packet.bytes(),
                flits,
                credit_delay_ticks,
                timing.clone(),
            );
            debug_assert_eq!(activity.queue_delay_ticks(), queue_delay_ticks);
            self.activity_log.push(activity);
            timings.push(timing);
            arrival_tick = reservation.arrival_tick;
        }

        Ok(FabricTransfer::new(
            packet,
            injection_tick,
            arrival_tick,
            timings,
        ))
    }
}

fn collect_lane_activities(records: &[FabricHopActivity]) -> Vec<FabricLaneActivity> {
    let mut activities = BTreeMap::<FabricLaneKey, FabricLaneActivity>::new();
    for record in records {
        let activity = record.lane_activity();
        activities
            .entry(FabricLaneKey::new(
                record.link().clone(),
                record.virtual_network(),
            ))
            .and_modify(|stored| *stored = stored.clone().merge_window(activity.clone()))
            .or_insert(activity);
    }
    activities.into_values().collect()
}

fn fabric_packet_node(packet: FabricPacketId) -> WaitForNode {
    WaitForNode::transaction(format!("fabric.packet.{}", packet.get()))
        .expect("fabric packet wait-for label is generated from numeric ids")
}

fn fabric_resource_node(
    link: &FabricLinkId,
    virtual_network: VirtualNetworkId,
    wait_kind: FabricWaitKind,
) -> WaitForNode {
    let link = wait_for_label_segment(link.as_str());
    let label = if wait_kind.is_virtual_network_scoped() {
        format!(
            "fabric.{}.vn.{}.{}",
            link,
            virtual_network.get(),
            wait_kind.resource_suffix()
        )
    } else {
        format!("fabric.{}.{}", link, wait_kind.resource_suffix())
    };
    WaitForNode::resource(label).expect("fabric resource wait-for label is sanitized")
}

fn record_wait_interval(
    graph: &mut WaitForGraph,
    wait: &FabricWaitRecord,
    first_tick: Tick,
    last_tick: Tick,
) {
    let source = fabric_packet_node(wait.packet);
    let target = fabric_resource_node(&wait.link, wait.virtual_network, wait.kind);
    graph
        .record_wait(
            source.clone(),
            target.clone(),
            wait.kind.edge_kind(),
            first_tick,
        )
        .expect("fabric wait-for labels are generated from typed ids");
    if last_tick != first_tick {
        graph
            .record_wait(source, target, wait.kind.edge_kind(), last_tick)
            .expect("fabric wait-for labels are generated from typed ids");
    }
}

fn wait_for_label_segment(label: &str) -> String {
    label
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':') {
                byte as char
            } else {
                '_'
            }
        })
        .collect()
}

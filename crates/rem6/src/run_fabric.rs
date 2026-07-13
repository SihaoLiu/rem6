use std::collections::{BTreeMap, BTreeSet};

use rem6_fabric::{
    FabricActivityProfile, FabricHopActivity, FabricLaneActivity, FabricLinkActivity, FabricLinkId,
    FabricModel, FabricPath, FabricPathHop, FabricRouterId, FabricRouterStage,
    FabricVirtualNetworkActivity, QosQueueArbiter, VirtualNetworkId,
};
use rem6_kernel::{WaitForBlockedNodeWindow, WaitForEdgeKindWindow, WaitForTargetNodeWindow};
use rem6_system::RiscvSystemRun;
use rem6_transport::{FabricQosGrantActivity, FabricQosGrantDirection, MemoryTransport};

use crate::{config::RunFabricRouterStageConfig, execute_error, Rem6CliError, RunFabricConfig};

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
    link_activities: Vec<FabricLinkActivity>,
    lane_activities: Vec<FabricLaneActivity>,
    hop_activities: Vec<FabricHopActivity>,
    qos_grant_activities: Vec<FabricQosGrantActivity>,
    router_activities: Vec<Rem6RunFabricRouterActivity>,
    wait_for_edge_count: u64,
    wait_for_edge_kind_windows: Vec<WaitForEdgeKindWindow>,
    wait_for_blocked_node_windows: Vec<WaitForBlockedNodeWindow>,
    wait_for_target_node_windows: Vec<WaitForTargetNodeWindow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RunFabricRouterActivity {
    router: String,
    input_port: u32,
    output_port: u32,
    virtual_channel: u16,
    transfer_count: u64,
    byte_count: u64,
    flit_count: u64,
    latency_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    first_tick: u64,
    last_tick: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Rem6RunFabricRouterActivityBuilder {
    transfer_count: u64,
    byte_count: u64,
    flit_count: u64,
    latency_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    first_tick: Option<u64>,
    last_tick: Option<u64>,
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
        let qos_grant_activities = transport.fabric_qos_grant_activities();
        let profile = FabricActivityProfile::from_lanes(lane_activities.iter());
        let router_activities = Rem6RunFabricRouterActivity::from_hops(&hop_activities);
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
            link_activities: FabricLinkActivity::from_lanes(lane_activities.iter()),
            lane_activities,
            hop_activities,
            qos_grant_activities,
            router_activities,
            wait_for_edge_count: 0,
            wait_for_edge_kind_windows: Vec::new(),
            wait_for_blocked_node_windows: Vec::new(),
            wait_for_target_node_windows: Vec::new(),
        }
    }

    pub(crate) fn with_wait_for_run(mut self, run: &RiscvSystemRun) -> Self {
        let edges = run.fabric_wait_for_edges();
        self.wait_for_edge_count = edges.len() as u64;
        self.wait_for_edge_kind_windows = WaitForEdgeKindWindow::from_edges(edges.clone());
        self.wait_for_blocked_node_windows = WaitForBlockedNodeWindow::from_edges(edges.clone());
        self.wait_for_target_node_windows = WaitForTargetNodeWindow::from_edges(edges);
        self
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

    pub(crate) fn link_activities(&self) -> &[FabricLinkActivity] {
        &self.link_activities
    }

    pub(crate) fn hop_activities(&self) -> &[FabricHopActivity] {
        &self.hop_activities
    }

    pub(crate) fn router_activities(&self) -> &[Rem6RunFabricRouterActivity] {
        &self.router_activities
    }

    pub(crate) fn qos_grant_activities(&self) -> &[FabricQosGrantActivity] {
        &self.qos_grant_activities
    }

    fn qos_grant_activities_for(
        &self,
        direction: FabricQosGrantDirection,
    ) -> impl Iterator<Item = &FabricQosGrantActivity> {
        self.qos_grant_activities
            .iter()
            .filter(move |activity| activity.direction() == direction)
    }

    pub(crate) fn qos_grant_count(&self, direction: FabricQosGrantDirection) -> u64 {
        self.qos_grant_activities_for(direction).count() as u64
    }

    pub(crate) fn qos_candidate_count(&self, direction: FabricQosGrantDirection) -> u64 {
        self.qos_grant_activities_for(direction)
            .map(|activity| activity.candidates().len() as u64)
            .sum()
    }

    pub(crate) fn qos_suppressed_count(&self, direction: FabricQosGrantDirection) -> u64 {
        self.qos_grant_activities_for(direction)
            .map(|activity| activity.suppressed().len() as u64)
            .sum()
    }

    pub(crate) fn qos_batch_count(&self, direction: FabricQosGrantDirection) -> u64 {
        self.qos_grant_activities_for(direction)
            .map(FabricQosGrantActivity::batch)
            .collect::<BTreeSet<_>>()
            .len() as u64
    }

    pub(crate) fn qos_max_candidate_count(&self, direction: FabricQosGrantDirection) -> u64 {
        self.qos_grant_activities_for(direction)
            .map(|activity| activity.candidates().len() as u64)
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn virtual_network_activities(&self) -> Vec<FabricVirtualNetworkActivity> {
        FabricVirtualNetworkActivity::from_lanes(self.lane_activities.iter())
    }

    pub(crate) const fn wait_for_edge_count(&self) -> u64 {
        self.wait_for_edge_count
    }

    pub(crate) fn wait_for_edge_kind_windows(&self) -> &[WaitForEdgeKindWindow] {
        &self.wait_for_edge_kind_windows
    }

    pub(crate) fn wait_for_blocked_node_windows(&self) -> &[WaitForBlockedNodeWindow] {
        &self.wait_for_blocked_node_windows
    }

    pub(crate) fn wait_for_target_node_windows(&self) -> &[WaitForTargetNodeWindow] {
        &self.wait_for_target_node_windows
    }
}

impl Rem6RunFabricRouterActivity {
    fn from_hops(hops: &[FabricHopActivity]) -> Vec<Self> {
        let mut summaries =
            BTreeMap::<(String, u32, u32, u16), Rem6RunFabricRouterActivityBuilder>::new();
        for hop in hops {
            let Some(router) = hop.router() else {
                continue;
            };
            summaries
                .entry((
                    router.router().as_str().to_owned(),
                    router.input_port(),
                    router.output_port(),
                    router.virtual_channel(),
                ))
                .or_default()
                .record(hop);
        }

        summaries
            .into_iter()
            .map(
                |((router, input_port, output_port, virtual_channel), summary)| Self {
                    router,
                    input_port,
                    output_port,
                    virtual_channel,
                    transfer_count: summary.transfer_count,
                    byte_count: summary.byte_count,
                    flit_count: summary.flit_count,
                    latency_ticks: summary.latency_ticks,
                    queue_delay_ticks: summary.queue_delay_ticks,
                    max_queue_delay_ticks: summary.max_queue_delay_ticks,
                    first_tick: summary.first_tick.unwrap_or(0),
                    last_tick: summary.last_tick.unwrap_or(0),
                },
            )
            .collect()
    }

    pub(crate) fn router(&self) -> &str {
        &self.router
    }

    pub(crate) const fn input_port(&self) -> u32 {
        self.input_port
    }

    pub(crate) const fn output_port(&self) -> u32 {
        self.output_port
    }

    pub(crate) const fn virtual_channel(&self) -> u16 {
        self.virtual_channel
    }

    pub(crate) const fn transfer_count(&self) -> u64 {
        self.transfer_count
    }

    pub(crate) const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub(crate) const fn flit_count(&self) -> u64 {
        self.flit_count
    }

    pub(crate) const fn latency_ticks(&self) -> u64 {
        self.latency_ticks
    }

    pub(crate) const fn queue_delay_ticks(&self) -> u64 {
        self.queue_delay_ticks
    }

    pub(crate) const fn max_queue_delay_ticks(&self) -> u64 {
        self.max_queue_delay_ticks
    }

    pub(crate) const fn first_tick(&self) -> u64 {
        self.first_tick
    }

    pub(crate) const fn last_tick(&self) -> u64 {
        self.last_tick
    }
}

impl Rem6RunFabricRouterActivityBuilder {
    fn record(&mut self, hop: &FabricHopActivity) {
        let Some(router) = hop.router() else {
            return;
        };
        self.transfer_count = self.transfer_count.saturating_add(1);
        self.byte_count = self.byte_count.saturating_add(hop.bytes());
        self.flit_count = self.flit_count.saturating_add(hop.flits());
        self.latency_ticks = self.latency_ticks.saturating_add(router.latency_ticks());
        self.queue_delay_ticks = self
            .queue_delay_ticks
            .saturating_add(router.queue_delay_ticks());
        self.max_queue_delay_ticks = self.max_queue_delay_ticks.max(router.queue_delay_ticks());
        self.first_tick = Some(
            self.first_tick
                .map_or(router.ready_tick(), |tick| tick.min(router.ready_tick())),
        );
        self.last_tick = Some(
            self.last_tick
                .map_or(router.depart_tick(), |tick| tick.max(router.depart_tick())),
        );
    }
}

pub(crate) fn run_memory_transport(config: Option<&RunFabricConfig>) -> MemoryTransport {
    match config {
        Some(fabric) => match fabric.qos_queue_policy() {
            Some(policy) => MemoryTransport::with_fabric_qos(
                FabricModel::new(),
                QosQueueArbiter::new(policy.to_qos_queue_policy_kind()),
            ),
            None => MemoryTransport::with_fabric(FabricModel::new()),
        },
        None => MemoryTransport::new(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunFabricPathDirection {
    Request,
    Response,
}

pub(crate) fn run_fabric_path(
    fabric: &RunFabricConfig,
    latency: u64,
    virtual_network: u16,
    direction: RunFabricPathDirection,
) -> Result<FabricPath, Rem6CliError> {
    let hops = fabric
        .hops()
        .iter()
        .map(|hop_config| {
            let link = FabricLinkId::new(hop_config.link()).map_err(execute_error)?;
            let hop = FabricPathHop::new(link, latency, hop_config.bandwidth_bytes_per_tick())
                .map_err(execute_error)?
                .with_virtual_network(VirtualNetworkId::new(virtual_network));
            let hop = match fabric.credit_depth() {
                Some(credit_depth) => hop.with_credit_depth(credit_depth).map_err(execute_error)?,
                None => hop,
            };
            match hop_config.router_stage() {
                Some(router_stage) => {
                    let router_stage = run_fabric_router_stage(router_stage, direction)?;
                    Ok(hop.with_router_stage(router_stage))
                }
                None => Ok(hop),
            }
        })
        .collect::<Result<Vec<_>, Rem6CliError>>()?;
    FabricPath::new(hops).map_err(execute_error)
}

fn run_fabric_router_stage(
    router_stage: &RunFabricRouterStageConfig,
    direction: RunFabricPathDirection,
) -> Result<FabricRouterStage, Rem6CliError> {
    let router = FabricRouterId::new(router_stage.router()).map_err(execute_error)?;
    let virtual_channel = match direction {
        RunFabricPathDirection::Request => router_stage.request_virtual_channel(),
        RunFabricPathDirection::Response => router_stage.response_virtual_channel(),
    };
    FabricRouterStage::new(
        router,
        router_stage.input_port(),
        router_stage.output_port(),
        virtual_channel,
        router_stage.latency(),
    )
    .map_err(execute_error)
}

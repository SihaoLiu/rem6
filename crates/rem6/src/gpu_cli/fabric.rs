use std::collections::BTreeSet;

use rem6_fabric::{
    FabricActivityProfile, FabricHopActivity, FabricLaneActivity, FabricLinkActivity, FabricLinkId,
    FabricModel, FabricPath, FabricPathHop, FabricVirtualNetworkActivity, VirtualNetworkId,
};
use rem6_transport::{MemoryRoute, MemoryRouteHop, MemoryTransport};

use super::{parse_u64_value, Rem6GpuRunConfig, GPU_RUN_GPU_PARTITION, GPU_RUN_MEMORY_PARTITION};
use crate::formatting::json_escape;
use crate::{execute_error, transport_endpoint, Rem6CliError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GpuFabricConfig {
    link: String,
    bandwidth_bytes_per_tick: u64,
    request_virtual_network: u16,
    response_virtual_network: u16,
    credit_depth: Option<u32>,
}

impl GpuFabricConfig {
    fn new(
        link: String,
        bandwidth_bytes_per_tick: u64,
        request_virtual_network: u16,
        response_virtual_network: u16,
        credit_depth: Option<u32>,
    ) -> Self {
        Self {
            link,
            bandwidth_bytes_per_tick,
            request_virtual_network,
            response_virtual_network,
            credit_depth,
        }
    }

    pub(crate) fn link(&self) -> &str {
        &self.link
    }

    pub(crate) const fn bandwidth_bytes_per_tick(&self) -> u64 {
        self.bandwidth_bytes_per_tick
    }

    pub(crate) const fn request_virtual_network(&self) -> u16 {
        self.request_virtual_network
    }

    pub(crate) const fn response_virtual_network(&self) -> u16 {
        self.response_virtual_network
    }

    pub(crate) const fn credit_depth(&self) -> Option<u32> {
        self.credit_depth
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6GpuFabricSummary {
    profile: FabricActivityProfile,
    lane_activities: Vec<FabricLaneActivity>,
    hop_activities: Vec<FabricHopActivity>,
}

impl Rem6GpuFabricSummary {
    pub(super) fn new(
        lane_activities: Vec<FabricLaneActivity>,
        hop_activities: Vec<FabricHopActivity>,
    ) -> Self {
        let profile = FabricActivityProfile::from_lanes(lane_activities.iter());
        Self {
            profile,
            lane_activities,
            hop_activities,
        }
    }

    pub(super) fn empty() -> Self {
        Self::new(Vec::new(), Vec::new())
    }

    pub(crate) fn active_lane_count(&self) -> usize {
        self.profile.active_lane_count()
    }

    pub(crate) fn active_virtual_network_count(&self) -> usize {
        self.lane_activities
            .iter()
            .map(FabricLaneActivity::virtual_network)
            .collect::<BTreeSet<_>>()
            .len()
    }

    pub(crate) fn transfer_count(&self) -> usize {
        self.profile.transfer_count()
    }

    pub(crate) fn byte_count(&self) -> u64 {
        self.profile.byte_count()
    }

    pub(crate) fn flit_count(&self) -> u64 {
        self.profile.flit_count()
    }

    pub(crate) fn occupied_ticks(&self) -> u64 {
        self.profile.occupied_ticks()
    }

    pub(crate) fn queue_delay_ticks(&self) -> u64 {
        self.profile.queue_delay_ticks()
    }

    pub(crate) fn max_queue_delay_ticks(&self) -> u64 {
        self.profile.max_queue_delay_ticks()
    }

    pub(crate) fn credit_delay_ticks(&self) -> u64 {
        self.profile.credit_delay_ticks()
    }

    pub(crate) fn max_credit_delay_ticks(&self) -> u64 {
        self.profile.max_credit_delay_ticks()
    }

    pub(crate) fn contended_lane_count(&self) -> usize {
        self.profile.contended_lane_count()
    }

    pub(crate) fn lane_activities(&self) -> &[FabricLaneActivity] {
        &self.lane_activities
    }

    pub(crate) fn link_activities(&self) -> Vec<FabricLinkActivity> {
        FabricLinkActivity::from_lanes(self.lane_activities.iter())
    }

    pub(crate) fn hop_activities(&self) -> &[FabricHopActivity] {
        &self.hop_activities
    }

    pub(crate) fn virtual_network_activities(&self) -> Vec<FabricVirtualNetworkActivity> {
        FabricVirtualNetworkActivity::from_lanes(self.lane_activities.iter())
    }
}

pub(super) fn gpu_memory_transport(config: &Rem6GpuRunConfig) -> MemoryTransport {
    match config.fabric() {
        Some(_) => MemoryTransport::with_fabric(FabricModel::new()),
        None => MemoryTransport::new(),
    }
}

pub(super) fn gpu_memory_route(config: &Rem6GpuRunConfig) -> Result<MemoryRoute, Rem6CliError> {
    let source = transport_endpoint("gpu.global".to_string())?;
    let target = transport_endpoint("memory".to_string())?;
    let Some(fabric) = config.fabric() else {
        return MemoryRoute::new(
            source,
            GPU_RUN_GPU_PARTITION,
            target,
            GPU_RUN_MEMORY_PARTITION,
            config.memory_route_delay(),
            config.memory_route_delay(),
        )
        .map_err(execute_error);
    };

    let hop = MemoryRouteHop::new(
        target,
        GPU_RUN_MEMORY_PARTITION,
        config.memory_route_delay(),
        config.memory_route_delay(),
    )
    .map_err(execute_error)?
    .with_request_fabric_path(gpu_fabric_path(
        fabric,
        config.memory_route_delay(),
        fabric.request_virtual_network(),
    )?)
    .with_response_fabric_path(gpu_fabric_path(
        fabric,
        config.memory_route_delay(),
        fabric.response_virtual_network(),
    )?);

    MemoryRoute::new_path(source, GPU_RUN_GPU_PARTITION, [hop])
        .map_err(execute_error)
        .map(|route| {
            route.with_virtual_networks(
                VirtualNetworkId::new(fabric.request_virtual_network()),
                VirtualNetworkId::new(fabric.response_virtual_network()),
            )
        })
}

fn gpu_fabric_path(
    fabric: &GpuFabricConfig,
    latency: u64,
    virtual_network: u16,
) -> Result<FabricPath, Rem6CliError> {
    let link = FabricLinkId::new(fabric.link()).map_err(execute_error)?;
    let hop = FabricPathHop::new(link, latency, fabric.bandwidth_bytes_per_tick())
        .map_err(execute_error)?
        .with_virtual_network(VirtualNetworkId::new(virtual_network));
    let hop = match fabric.credit_depth() {
        Some(credit_depth) => hop.with_credit_depth(credit_depth).map_err(execute_error)?,
        None => hop,
    };
    FabricPath::new([hop]).map_err(execute_error)
}

pub(super) fn gpu_fabric_summary_json(
    config: Option<&GpuFabricConfig>,
    summary: &Rem6GpuFabricSummary,
) -> String {
    let Some(config) = config else {
        return "null".to_string();
    };
    let credit_depth = config
        .credit_depth()
        .map(|depth| depth.to_string())
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"link\":\"{}\",\"bandwidth_bytes_per_tick\":{},\"request_virtual_network\":{},\"response_virtual_network\":{},\"credit_depth\":{},\"active_lanes\":{},\"active_virtual_networks\":{},\"transfers\":{},\"bytes\":{},\"flits\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"contended_lanes\":{},\"link_activities\":[{}],\"lane_activities\":[{}],\"hop_activities\":[{}]}}",
        json_escape(config.link()),
        config.bandwidth_bytes_per_tick(),
        config.request_virtual_network(),
        config.response_virtual_network(),
        credit_depth,
        summary.active_lane_count(),
        summary.active_virtual_network_count(),
        summary.transfer_count(),
        summary.byte_count(),
        summary.flit_count(),
        summary.occupied_ticks(),
        summary.queue_delay_ticks(),
        summary.max_queue_delay_ticks(),
        summary.credit_delay_ticks(),
        summary.max_credit_delay_ticks(),
        summary.contended_lane_count(),
        gpu_fabric_link_activities_json(summary),
        gpu_fabric_lane_activities_json(summary),
        gpu_fabric_hop_activities_json(summary),
    )
}

fn gpu_fabric_link_activities_json(summary: &Rem6GpuFabricSummary) -> String {
    summary
        .link_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"active_virtual_networks\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"contended_virtual_networks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.active_virtual_network_count(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.flit_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.max_credit_delay_ticks(),
                activity.contended_virtual_network_count(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn gpu_fabric_lane_activities_json(summary: &Rem6GpuFabricSummary) -> String {
    summary
        .lane_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"virtual_network\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.flit_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.max_credit_delay_ticks(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn gpu_fabric_hop_activities_json(summary: &Rem6GpuFabricSummary) -> String {
    summary
        .hop_activities()
        .iter()
        .map(|activity| {
            let router = gpu_fabric_hop_router_json(activity);
            format!(
                "{{\"packet\":{},\"hop_index\":{},\"link\":\"{}\",\"virtual_network\":{},\"router\":{},\"bytes\":{},\"flits\":{},\"ready_tick\":{},\"start_tick\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"depart_tick\":{},\"arrival_tick\":{}}}",
                activity.packet().get(),
                activity.hop_index(),
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                router,
                activity.bytes(),
                activity.flits(),
                activity.ready_tick(),
                activity.start_tick(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.depart_tick(),
                activity.arrival_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn gpu_fabric_hop_router_json(activity: &FabricHopActivity) -> String {
    match activity.router() {
        Some(router) => format!(
            "{{\"router\":\"{}\",\"input_port\":{},\"output_port\":{},\"virtual_channel\":{},\"ready_tick\":{},\"start_tick\":{},\"latency_ticks\":{},\"depart_tick\":{},\"queue_delay_ticks\":{}}}",
            json_escape(router.router().as_str()),
            router.input_port(),
            router.output_port(),
            router.virtual_channel(),
            router.ready_tick(),
            router.start_tick(),
            router.latency_ticks(),
            router.depart_tick(),
            router.queue_delay_ticks(),
        ),
        None => "null".to_string(),
    }
}

pub(super) fn parse_gpu_fabric_bandwidth(value: &str) -> Result<u64, Rem6CliError> {
    parse_u64_value("--fabric-bandwidth-bytes-per-tick", value.to_string())
        .ok()
        .filter(|bandwidth| *bandwidth > 0)
        .ok_or_else(|| Rem6CliError::InvalidGpuRunFabricBandwidth {
            value: value.to_string(),
        })
}

pub(super) fn parse_gpu_fabric_virtual_network(value: &str) -> Result<u16, Rem6CliError> {
    parse_u64_value("--fabric-virtual-network", value.to_string())
        .ok()
        .and_then(|network| u16::try_from(network).ok())
        .ok_or_else(|| Rem6CliError::InvalidGpuRunFabricVirtualNetwork {
            value: value.to_string(),
        })
}

pub(super) fn parse_gpu_fabric_credit_depth(value: &str) -> Result<u32, Rem6CliError> {
    parse_u64_value("--fabric-credit-depth", value.to_string())
        .ok()
        .filter(|depth| *depth > 0)
        .and_then(|depth| u32::try_from(depth).ok())
        .ok_or_else(|| Rem6CliError::InvalidGpuRunFabricCreditDepth {
            value: value.to_string(),
        })
}

pub(super) fn gpu_fabric_config_from_parts(
    link: Option<String>,
    bandwidth_bytes_per_tick: Option<u64>,
    request_virtual_network: Option<u16>,
    response_virtual_network: Option<u16>,
    credit_depth: Option<u32>,
) -> Result<Option<GpuFabricConfig>, Rem6CliError> {
    if link.is_some() && bandwidth_bytes_per_tick.is_none() {
        return Err(Rem6CliError::MissingRequiredFlag {
            flag: "--fabric-bandwidth-bytes-per-tick",
        });
    }
    if link.is_none() && bandwidth_bytes_per_tick.is_some() {
        return Err(Rem6CliError::MissingRequiredFlag {
            flag: "--fabric-link",
        });
    }
    if link.is_none()
        && (request_virtual_network.is_some()
            || response_virtual_network.is_some()
            || credit_depth.is_some())
    {
        return Err(Rem6CliError::MissingRequiredFlag {
            flag: "--fabric-link",
        });
    }

    Ok(match (link, bandwidth_bytes_per_tick) {
        (Some(link), Some(bandwidth_bytes_per_tick)) => Some(GpuFabricConfig::new(
            link,
            bandwidth_bytes_per_tick,
            request_virtual_network.unwrap_or(0),
            response_virtual_network.unwrap_or(0),
            credit_depth,
        )),
        _ => None,
    })
}

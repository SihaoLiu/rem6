use super::parse::{parse_number, parse_positive_u64};
use crate::Rem6CliError;

use rem6_fabric::QosQueuePolicyKind;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RunFabricConfigParts {
    link: Option<Vec<String>>,
    bandwidth_bytes_per_tick: Option<Vec<u64>>,
    request_virtual_network: Option<u16>,
    response_virtual_network: Option<u16>,
    credit_depth: Option<u32>,
    router: Option<Vec<String>>,
    router_input_port: Option<Vec<u32>>,
    router_output_port: Option<Vec<u32>>,
    router_virtual_channel: Option<Vec<u16>>,
    request_router_virtual_channel: Option<Vec<u16>>,
    response_router_virtual_channel: Option<Vec<u16>>,
    router_latency: Option<Vec<u64>>,
    qos_queue_policy: Option<RunFabricQosQueuePolicy>,
}

impl RunFabricConfigParts {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        link: Option<String>,
        bandwidth_bytes_per_tick: Option<u64>,
        request_virtual_network: Option<u16>,
        response_virtual_network: Option<u16>,
        credit_depth: Option<u32>,
        router: Option<String>,
        router_input_port: Option<u32>,
        router_output_port: Option<u32>,
        router_virtual_channel: Option<u16>,
        request_router_virtual_channel: Option<u16>,
        response_router_virtual_channel: Option<u16>,
        router_latency: Option<u64>,
        qos_queue_policy: Option<String>,
    ) -> Result<Self, Rem6CliError> {
        if bandwidth_bytes_per_tick == Some(0) {
            return Err(Rem6CliError::InvalidRunFabricBandwidth {
                value: "0".to_string(),
            });
        }
        if credit_depth == Some(0) {
            return Err(Rem6CliError::InvalidRunFabricCreditDepth {
                value: "0".to_string(),
            });
        }
        if router_latency == Some(0) {
            return Err(Rem6CliError::InvalidRunFabricRouterLatency {
                value: "0".to_string(),
            });
        }
        Ok(Self {
            link: link.map(|link| parse_run_fabric_link_list(&link)),
            bandwidth_bytes_per_tick: bandwidth_bytes_per_tick.map(|value| vec![value]),
            request_virtual_network,
            response_virtual_network,
            credit_depth,
            router: router
                .as_deref()
                .map(parse_run_fabric_router_list)
                .transpose()?,
            router_input_port: router_input_port.map(|value| vec![value]),
            router_output_port: router_output_port.map(|value| vec![value]),
            router_virtual_channel: router_virtual_channel.map(|value| vec![value]),
            request_router_virtual_channel: request_router_virtual_channel.map(|value| vec![value]),
            response_router_virtual_channel: response_router_virtual_channel
                .map(|value| vec![value]),
            router_latency: router_latency.map(|value| vec![value]),
            qos_queue_policy: qos_queue_policy
                .as_deref()
                .map(parse_run_fabric_qos_queue_policy)
                .transpose()?,
        })
    }

    pub(super) fn is_set(&self) -> bool {
        self.link.is_some()
            || self.bandwidth_bytes_per_tick.is_some()
            || self.request_virtual_network.is_some()
            || self.response_virtual_network.is_some()
            || self.credit_depth.is_some()
            || self.router.is_some()
            || self.router_input_port.is_some()
            || self.router_output_port.is_some()
            || self.router_virtual_channel.is_some()
            || self.request_router_virtual_channel.is_some()
            || self.response_router_virtual_channel.is_some()
            || self.router_latency.is_some()
            || self.qos_queue_policy.is_some()
    }

    pub(super) fn set_link(&mut self, link: String) {
        self.link = Some(parse_run_fabric_link_list(&link));
    }

    pub(super) fn set_bandwidth(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.bandwidth_bytes_per_tick = Some(parse_run_fabric_bandwidth_list(value)?);
        Ok(())
    }

    pub(super) fn set_request_virtual_network(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.request_virtual_network = Some(parse_run_fabric_virtual_network(value)?);
        Ok(())
    }

    pub(super) fn set_response_virtual_network(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.response_virtual_network = Some(parse_run_fabric_virtual_network(value)?);
        Ok(())
    }

    pub(super) fn set_credit_depth(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.credit_depth = Some(parse_run_fabric_credit_depth(value)?);
        Ok(())
    }

    pub(super) fn set_router(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router = Some(parse_run_fabric_router_list(value)?);
        Ok(())
    }

    pub(super) fn set_router_input_port(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router_input_port = Some(parse_run_fabric_router_port_list(value)?);
        Ok(())
    }

    pub(super) fn set_router_output_port(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router_output_port = Some(parse_run_fabric_router_port_list(value)?);
        Ok(())
    }

    pub(super) fn set_router_virtual_channel(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router_virtual_channel = Some(parse_run_fabric_router_virtual_channel_list(value)?);
        Ok(())
    }

    pub(super) fn set_request_router_virtual_channel(
        &mut self,
        value: &str,
    ) -> Result<(), Rem6CliError> {
        self.request_router_virtual_channel =
            Some(parse_run_fabric_router_virtual_channel_list(value)?);
        Ok(())
    }

    pub(super) fn set_response_router_virtual_channel(
        &mut self,
        value: &str,
    ) -> Result<(), Rem6CliError> {
        self.response_router_virtual_channel =
            Some(parse_run_fabric_router_virtual_channel_list(value)?);
        Ok(())
    }

    pub(super) fn set_router_latency(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router_latency = Some(parse_run_fabric_router_latency_list(value)?);
        Ok(())
    }

    pub(super) fn set_qos_queue_policy(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.qos_queue_policy = Some(parse_run_fabric_qos_queue_policy(value)?);
        Ok(())
    }

    pub(super) fn apply_cache_fabric_dram_defaults(&mut self) {
        self.link.get_or_insert_with(|| vec!["cpu_mem".to_string()]);
        self.bandwidth_bytes_per_tick
            .get_or_insert_with(|| vec![64]);
        self.request_virtual_network.get_or_insert(1);
        self.response_virtual_network.get_or_insert(2);
        self.credit_depth.get_or_insert(4);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunFabricRouterStageConfig {
    router: String,
    input_port: u32,
    output_port: u32,
    virtual_channel: u16,
    request_virtual_channel: u16,
    response_virtual_channel: u16,
    latency: u64,
}

impl RunFabricRouterStageConfig {
    const fn new(
        router: String,
        input_port: u32,
        output_port: u32,
        virtual_channel: u16,
        request_virtual_channel: u16,
        response_virtual_channel: u16,
        latency: u64,
    ) -> Self {
        Self {
            router,
            input_port,
            output_port,
            virtual_channel,
            request_virtual_channel,
            response_virtual_channel,
            latency,
        }
    }

    pub fn router(&self) -> &str {
        &self.router
    }

    pub const fn input_port(&self) -> u32 {
        self.input_port
    }

    pub const fn output_port(&self) -> u32 {
        self.output_port
    }

    pub const fn virtual_channel(&self) -> u16 {
        self.virtual_channel
    }

    pub const fn request_virtual_channel(&self) -> u16 {
        self.request_virtual_channel
    }

    pub const fn response_virtual_channel(&self) -> u16 {
        self.response_virtual_channel
    }

    pub const fn latency(&self) -> u64 {
        self.latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunFabricHopConfig {
    link: String,
    bandwidth_bytes_per_tick: u64,
    router_stage: Option<RunFabricRouterStageConfig>,
}

impl RunFabricHopConfig {
    const fn new(
        link: String,
        bandwidth_bytes_per_tick: u64,
        router_stage: Option<RunFabricRouterStageConfig>,
    ) -> Self {
        Self {
            link,
            bandwidth_bytes_per_tick,
            router_stage,
        }
    }

    pub(crate) fn link(&self) -> &str {
        &self.link
    }

    pub(crate) fn bandwidth_bytes_per_tick(&self) -> u64 {
        self.bandwidth_bytes_per_tick
    }

    pub(crate) fn router_stage(&self) -> Option<&RunFabricRouterStageConfig> {
        self.router_stage.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunFabricConfig {
    hops: Vec<RunFabricHopConfig>,
    request_virtual_network: u16,
    response_virtual_network: u16,
    credit_depth: Option<u32>,
    qos_queue_policy: Option<RunFabricQosQueuePolicy>,
}

impl RunFabricConfig {
    pub(super) fn new(
        hops: Vec<RunFabricHopConfig>,
        request_virtual_network: u16,
        response_virtual_network: u16,
        credit_depth: Option<u32>,
        qos_queue_policy: Option<RunFabricQosQueuePolicy>,
    ) -> Self {
        Self {
            hops,
            request_virtual_network,
            response_virtual_network,
            credit_depth,
            qos_queue_policy,
        }
    }

    pub fn link(&self) -> &str {
        self.hops.first().map_or("", RunFabricHopConfig::link)
    }

    pub fn bandwidth_bytes_per_tick(&self) -> u64 {
        match self.hops.first() {
            Some(hop) => hop.bandwidth_bytes_per_tick(),
            None => 0,
        }
    }

    pub const fn request_virtual_network(&self) -> u16 {
        self.request_virtual_network
    }

    pub const fn response_virtual_network(&self) -> u16 {
        self.response_virtual_network
    }

    pub const fn credit_depth(&self) -> Option<u32> {
        self.credit_depth
    }

    pub fn router_stage(&self) -> Option<&RunFabricRouterStageConfig> {
        self.hops.first().and_then(RunFabricHopConfig::router_stage)
    }

    pub(crate) fn hops(&self) -> &[RunFabricHopConfig] {
        &self.hops
    }

    pub const fn qos_queue_policy(&self) -> Option<RunFabricQosQueuePolicy> {
        self.qos_queue_policy
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunFabricQosQueuePolicy {
    Fifo,
    Lifo,
    LeastRecentlyGranted,
}

impl RunFabricQosQueuePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fifo => "fifo",
            Self::Lifo => "lifo",
            Self::LeastRecentlyGranted => "least-recently-granted",
        }
    }

    pub const fn to_qos_queue_policy_kind(self) -> QosQueuePolicyKind {
        match self {
            Self::Fifo => QosQueuePolicyKind::Fifo,
            Self::Lifo => QosQueuePolicyKind::Lifo,
            Self::LeastRecentlyGranted => QosQueuePolicyKind::LeastRecentlyGranted,
        }
    }
}

pub(super) fn run_fabric_config_from_parts(
    parts: RunFabricConfigParts,
) -> Result<Option<RunFabricConfig>, Rem6CliError> {
    if parts.link.is_none() {
        if parts.is_set() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--fabric-link",
            });
        }
        return Ok(None);
    }
    let links = parts.link.expect("checked above");
    let hop_count = links.len();
    let bandwidths = expand_required_fabric_values(
        parts.bandwidth_bytes_per_tick,
        hop_count,
        "--fabric-bandwidth-bytes-per-tick",
        invalid_run_fabric_bandwidth_list,
    )?;
    let router_stages = run_fabric_router_stage_configs(
        hop_count,
        parts.router,
        parts.router_input_port,
        parts.router_output_port,
        parts.router_virtual_channel,
        parts.request_router_virtual_channel,
        parts.response_router_virtual_channel,
        parts.router_latency,
    )?;
    let hops = links
        .into_iter()
        .zip(bandwidths)
        .zip(router_stages)
        .map(|((link, bandwidth_bytes_per_tick), router_stage)| {
            RunFabricHopConfig::new(link, bandwidth_bytes_per_tick, router_stage)
        })
        .collect();

    Ok(Some(RunFabricConfig::new(
        hops,
        parts.request_virtual_network.unwrap_or(0),
        parts.response_virtual_network.unwrap_or(0),
        parts.credit_depth,
        parts.qos_queue_policy,
    )))
}

pub(super) fn parse_run_fabric_bandwidth(value: &str) -> Result<u64, Rem6CliError> {
    parse_positive_u64(value).ok_or_else(|| Rem6CliError::InvalidRunFabricBandwidth {
        value: value.to_string(),
    })
}

fn parse_run_fabric_link_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_run_fabric_bandwidth_list(value: &str) -> Result<Vec<u64>, Rem6CliError> {
    parse_run_fabric_list(value, parse_run_fabric_bandwidth)
}

pub(super) fn parse_run_fabric_virtual_network(value: &str) -> Result<u16, Rem6CliError> {
    value
        .parse()
        .map_err(|_| Rem6CliError::InvalidRunFabricVirtualNetwork {
            value: value.to_string(),
        })
}

pub(super) fn parse_run_fabric_credit_depth(value: &str) -> Result<u32, Rem6CliError> {
    value
        .parse()
        .ok()
        .filter(|depth| *depth > 0)
        .ok_or_else(|| Rem6CliError::InvalidRunFabricCreditDepth {
            value: value.to_string(),
        })
}

fn parse_run_fabric_router(value: &str) -> Result<String, Rem6CliError> {
    if value.is_empty() {
        return Err(Rem6CliError::InvalidRunFabricRouter {
            value: value.to_string(),
        });
    }
    Ok(value.to_string())
}

fn parse_run_fabric_router_list(value: &str) -> Result<Vec<String>, Rem6CliError> {
    parse_run_fabric_list(value, parse_run_fabric_router)
}

fn parse_run_fabric_router_port(value: &str) -> Result<u32, Rem6CliError> {
    parse_number(value)
        .and_then(|port| u32::try_from(port).ok())
        .ok_or_else(|| Rem6CliError::InvalidRunFabricRouterPort {
            value: value.to_string(),
        })
}

fn parse_run_fabric_router_port_list(value: &str) -> Result<Vec<u32>, Rem6CliError> {
    parse_run_fabric_list(value, parse_run_fabric_router_port)
}

fn parse_run_fabric_router_virtual_channel(value: &str) -> Result<u16, Rem6CliError> {
    parse_number(value)
        .and_then(|channel| u16::try_from(channel).ok())
        .ok_or_else(|| Rem6CliError::InvalidRunFabricRouterVirtualChannel {
            value: value.to_string(),
        })
}

fn parse_run_fabric_router_virtual_channel_list(value: &str) -> Result<Vec<u16>, Rem6CliError> {
    parse_run_fabric_list(value, parse_run_fabric_router_virtual_channel)
}

fn parse_run_fabric_router_latency(value: &str) -> Result<u64, Rem6CliError> {
    parse_positive_u64(value).ok_or_else(|| Rem6CliError::InvalidRunFabricRouterLatency {
        value: value.to_string(),
    })
}

fn parse_run_fabric_router_latency_list(value: &str) -> Result<Vec<u64>, Rem6CliError> {
    parse_run_fabric_list(value, parse_run_fabric_router_latency)
}

fn parse_run_fabric_qos_queue_policy(value: &str) -> Result<RunFabricQosQueuePolicy, Rem6CliError> {
    match value {
        "fifo" => Ok(RunFabricQosQueuePolicy::Fifo),
        "lifo" => Ok(RunFabricQosQueuePolicy::Lifo),
        "least-recently-granted" | "lrg" => Ok(RunFabricQosQueuePolicy::LeastRecentlyGranted),
        _ => Err(Rem6CliError::InvalidRunFabricQosQueuePolicy {
            value: value.to_string(),
        }),
    }
}

fn parse_run_fabric_list<T>(
    value: &str,
    parse: fn(&str) -> Result<T, Rem6CliError>,
) -> Result<Vec<T>, Rem6CliError> {
    value.split(',').map(str::trim).map(parse).collect()
}

fn run_fabric_router_stage_configs(
    hop_count: usize,
    router: Option<Vec<String>>,
    input_port: Option<Vec<u32>>,
    output_port: Option<Vec<u32>>,
    virtual_channel: Option<Vec<u16>>,
    request_virtual_channel: Option<Vec<u16>>,
    response_virtual_channel: Option<Vec<u16>>,
    latency: Option<Vec<u64>>,
) -> Result<Vec<Option<RunFabricRouterStageConfig>>, Rem6CliError> {
    let has_router_stage = router.is_some()
        || input_port.is_some()
        || output_port.is_some()
        || virtual_channel.is_some()
        || request_virtual_channel.is_some()
        || response_virtual_channel.is_some()
        || latency.is_some();
    if !has_router_stage {
        return Ok(vec![None; hop_count]);
    }
    let routers = expand_required_fabric_values(
        router,
        hop_count,
        "--fabric-router",
        invalid_run_fabric_router_list,
    )?;
    let input_ports = expand_required_fabric_values(
        input_port,
        hop_count,
        "--fabric-router-input-port",
        invalid_run_fabric_router_port_list,
    )?;
    let output_ports = expand_required_fabric_values(
        output_port,
        hop_count,
        "--fabric-router-output-port",
        invalid_run_fabric_router_port_list,
    )?;
    let virtual_channels = expand_required_fabric_values(
        virtual_channel,
        hop_count,
        "--fabric-router-virtual-channel",
        invalid_run_fabric_router_virtual_channel_list,
    )?;
    let request_virtual_channels = expand_optional_fabric_values(
        request_virtual_channel,
        &virtual_channels,
        hop_count,
        invalid_run_fabric_router_virtual_channel_list,
    )?;
    let response_virtual_channels = expand_optional_fabric_values(
        response_virtual_channel,
        &virtual_channels,
        hop_count,
        invalid_run_fabric_router_virtual_channel_list,
    )?;
    let latencies = expand_required_fabric_values(
        latency,
        hop_count,
        "--fabric-router-latency",
        invalid_run_fabric_router_latency_list,
    )?;

    Ok(routers
        .into_iter()
        .zip(input_ports)
        .zip(output_ports)
        .zip(virtual_channels)
        .zip(request_virtual_channels)
        .zip(response_virtual_channels)
        .zip(latencies)
        .map(
            |(
                (((((router, input_port), output_port), virtual_channel), request_vc), response_vc),
                latency,
            )| {
                Some(RunFabricRouterStageConfig::new(
                    router,
                    input_port,
                    output_port,
                    virtual_channel,
                    request_vc,
                    response_vc,
                    latency,
                ))
            },
        )
        .collect())
}

fn expand_required_fabric_values<T: Clone>(
    values: Option<Vec<T>>,
    hop_count: usize,
    flag: &'static str,
    invalid: fn(Vec<T>) -> Rem6CliError,
) -> Result<Vec<T>, Rem6CliError> {
    let values = values.ok_or(Rem6CliError::MissingRequiredFlag { flag })?;
    expand_fabric_values(values, hop_count, invalid)
}

fn expand_optional_fabric_values<T: Clone>(
    values: Option<Vec<T>>,
    defaults: &[T],
    hop_count: usize,
    invalid: fn(Vec<T>) -> Rem6CliError,
) -> Result<Vec<T>, Rem6CliError> {
    match values {
        Some(values) => expand_fabric_values(values, hop_count, invalid),
        None => Ok(defaults.to_vec()),
    }
}

fn expand_fabric_values<T: Clone>(
    values: Vec<T>,
    hop_count: usize,
    invalid: fn(Vec<T>) -> Rem6CliError,
) -> Result<Vec<T>, Rem6CliError> {
    if values.len() == hop_count {
        return Ok(values);
    }
    if values.len() == 1 {
        return Ok(vec![values[0].clone(); hop_count]);
    }
    Err(invalid(values))
}

fn invalid_run_fabric_bandwidth_list(values: Vec<u64>) -> Rem6CliError {
    Rem6CliError::InvalidRunFabricBandwidth {
        value: join_run_fabric_values(values),
    }
}

fn invalid_run_fabric_router_list(values: Vec<String>) -> Rem6CliError {
    Rem6CliError::InvalidRunFabricRouter {
        value: values.join(","),
    }
}

fn invalid_run_fabric_router_port_list(values: Vec<u32>) -> Rem6CliError {
    Rem6CliError::InvalidRunFabricRouterPort {
        value: join_run_fabric_values(values),
    }
}

fn invalid_run_fabric_router_virtual_channel_list(values: Vec<u16>) -> Rem6CliError {
    Rem6CliError::InvalidRunFabricRouterVirtualChannel {
        value: join_run_fabric_values(values),
    }
}

fn invalid_run_fabric_router_latency_list(values: Vec<u64>) -> Rem6CliError {
    Rem6CliError::InvalidRunFabricRouterLatency {
        value: join_run_fabric_values(values),
    }
}

fn join_run_fabric_values<T: ToString>(values: Vec<T>) -> String {
    values
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_fabric_config_parts_build_router_stage() {
        let config = run_fabric_config_from_parts(
            RunFabricConfigParts::new(
                Some("cpu_mem".to_string()),
                Some(8),
                Some(3),
                Some(4),
                Some(2),
                Some("router0".to_string()),
                Some(1),
                Some(2),
                Some(3),
                Some(11),
                Some(13),
                Some(5),
                Some("least-recently-granted".to_string()),
            )
            .unwrap(),
        )
        .unwrap()
        .unwrap();
        let router_stage = config.router_stage().unwrap();

        assert_eq!(config.link(), "cpu_mem");
        assert_eq!(config.bandwidth_bytes_per_tick(), 8);
        assert_eq!(config.request_virtual_network(), 3);
        assert_eq!(config.response_virtual_network(), 4);
        assert_eq!(config.credit_depth(), Some(2));
        assert_eq!(router_stage.router(), "router0");
        assert_eq!(router_stage.input_port(), 1);
        assert_eq!(router_stage.output_port(), 2);
        assert_eq!(router_stage.virtual_channel(), 3);
        assert_eq!(router_stage.request_virtual_channel(), 11);
        assert_eq!(router_stage.response_virtual_channel(), 13);
        assert_eq!(router_stage.latency(), 5);
        assert_eq!(
            config.qos_queue_policy(),
            Some(RunFabricQosQueuePolicy::LeastRecentlyGranted)
        );
    }

    #[test]
    fn run_fabric_config_parts_build_multi_hop_router_stages() {
        let mut parts = RunFabricConfigParts::default();
        parts.set_link("cpu_r0,r0_mem".to_string());
        parts.set_bandwidth("8,4").unwrap();
        parts.set_router("router0,router1").unwrap();
        parts.set_router_input_port("1,2").unwrap();
        parts.set_router_output_port("2,3").unwrap();
        parts.set_router_virtual_channel("7,8").unwrap();
        parts.set_router_latency("3,5").unwrap();

        let config = run_fabric_config_from_parts(parts).unwrap().unwrap();
        let hops = config.hops();

        assert_eq!(hops.len(), 2);
        assert_eq!(hops[0].link(), "cpu_r0");
        assert_eq!(hops[0].bandwidth_bytes_per_tick(), 8);
        let first_router = hops[0].router_stage().unwrap();
        assert_eq!(first_router.router(), "router0");
        assert_eq!(first_router.input_port(), 1);
        assert_eq!(first_router.output_port(), 2);
        assert_eq!(first_router.virtual_channel(), 7);
        assert_eq!(first_router.request_virtual_channel(), 7);
        assert_eq!(first_router.response_virtual_channel(), 7);
        assert_eq!(first_router.latency(), 3);
        assert_eq!(hops[1].link(), "r0_mem");
        assert_eq!(hops[1].bandwidth_bytes_per_tick(), 4);
        let second_router = hops[1].router_stage().unwrap();
        assert_eq!(second_router.router(), "router1");
        assert_eq!(second_router.input_port(), 2);
        assert_eq!(second_router.output_port(), 3);
        assert_eq!(second_router.virtual_channel(), 8);
        assert_eq!(second_router.request_virtual_channel(), 8);
        assert_eq!(second_router.response_virtual_channel(), 8);
        assert_eq!(second_router.latency(), 5);
    }

    #[test]
    fn run_fabric_config_parts_expands_scalar_hop_fields() {
        let mut parts = RunFabricConfigParts::default();
        parts.set_link("cpu_r0,r0_mem".to_string());
        parts.set_bandwidth("8").unwrap();
        parts.set_router("router0").unwrap();
        parts.set_router_input_port("1").unwrap();
        parts.set_router_output_port("2").unwrap();
        parts.set_router_virtual_channel("7").unwrap();
        parts.set_router_latency("3").unwrap();

        let config = run_fabric_config_from_parts(parts).unwrap().unwrap();

        assert_eq!(config.hops().len(), 2);
        assert!(config
            .hops()
            .iter()
            .all(|hop| hop.bandwidth_bytes_per_tick() == 8));
        assert!(config.hops().iter().all(|hop| {
            hop.router_stage().is_some_and(|stage| {
                stage.router() == "router0"
                    && stage.input_port() == 1
                    && stage.output_port() == 2
                    && stage.virtual_channel() == 7
                    && stage.latency() == 3
            })
        }));
    }

    #[test]
    fn run_fabric_config_parts_reject_partial_router_stage() {
        let error = run_fabric_config_from_parts(
            RunFabricConfigParts::new(
                Some("cpu_mem".to_string()),
                Some(8),
                None,
                None,
                None,
                Some("router0".to_string()),
                Some(1),
                None,
                Some(3),
                None,
                None,
                Some(5),
                None,
            )
            .unwrap(),
        )
        .unwrap_err();

        assert_eq!(
            error,
            Rem6CliError::MissingRequiredFlag {
                flag: "--fabric-router-output-port"
            }
        );
    }

    #[test]
    fn run_fabric_config_parts_reject_invalid_qos_queue_policy() {
        let error = RunFabricConfigParts::new(
            Some("cpu_mem".to_string()),
            Some(8),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("random".to_string()),
        )
        .unwrap_err();

        assert_eq!(
            error,
            Rem6CliError::InvalidRunFabricQosQueuePolicy {
                value: "random".to_string()
            }
        );
    }
}

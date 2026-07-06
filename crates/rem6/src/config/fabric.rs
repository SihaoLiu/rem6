use super::parse::{parse_number, parse_positive_u64};
use crate::Rem6CliError;

use rem6_fabric::QosQueuePolicyKind;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RunFabricConfigParts {
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
        if matches!(router.as_deref(), Some("")) {
            return Err(Rem6CliError::InvalidRunFabricRouter {
                value: String::new(),
            });
        }
        if router_latency == Some(0) {
            return Err(Rem6CliError::InvalidRunFabricRouterLatency {
                value: "0".to_string(),
            });
        }
        Ok(Self {
            link,
            bandwidth_bytes_per_tick,
            request_virtual_network,
            response_virtual_network,
            credit_depth,
            router,
            router_input_port,
            router_output_port,
            router_virtual_channel,
            request_router_virtual_channel,
            response_router_virtual_channel,
            router_latency,
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
        self.link = Some(link);
    }

    pub(super) fn set_bandwidth(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.bandwidth_bytes_per_tick = Some(parse_run_fabric_bandwidth(value)?);
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
        self.router = Some(parse_run_fabric_router(value)?);
        Ok(())
    }

    pub(super) fn set_router_input_port(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router_input_port = Some(parse_run_fabric_router_port(value)?);
        Ok(())
    }

    pub(super) fn set_router_output_port(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router_output_port = Some(parse_run_fabric_router_port(value)?);
        Ok(())
    }

    pub(super) fn set_router_virtual_channel(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router_virtual_channel = Some(parse_run_fabric_router_virtual_channel(value)?);
        Ok(())
    }

    pub(super) fn set_request_router_virtual_channel(
        &mut self,
        value: &str,
    ) -> Result<(), Rem6CliError> {
        self.request_router_virtual_channel = Some(parse_run_fabric_router_virtual_channel(value)?);
        Ok(())
    }

    pub(super) fn set_response_router_virtual_channel(
        &mut self,
        value: &str,
    ) -> Result<(), Rem6CliError> {
        self.response_router_virtual_channel =
            Some(parse_run_fabric_router_virtual_channel(value)?);
        Ok(())
    }

    pub(super) fn set_router_latency(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.router_latency = Some(parse_run_fabric_router_latency(value)?);
        Ok(())
    }

    pub(super) fn set_qos_queue_policy(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.qos_queue_policy = Some(parse_run_fabric_qos_queue_policy(value)?);
        Ok(())
    }

    pub(super) fn apply_cache_fabric_dram_defaults(&mut self) {
        self.link.get_or_insert_with(|| "cpu_mem".to_string());
        self.bandwidth_bytes_per_tick.get_or_insert(64);
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
pub struct RunFabricConfig {
    link: String,
    bandwidth_bytes_per_tick: u64,
    request_virtual_network: u16,
    response_virtual_network: u16,
    credit_depth: Option<u32>,
    router_stage: Option<RunFabricRouterStageConfig>,
    qos_queue_policy: Option<RunFabricQosQueuePolicy>,
}

impl RunFabricConfig {
    pub(super) fn new(
        link: String,
        bandwidth_bytes_per_tick: u64,
        request_virtual_network: u16,
        response_virtual_network: u16,
        credit_depth: Option<u32>,
        router_stage: Option<RunFabricRouterStageConfig>,
        qos_queue_policy: Option<RunFabricQosQueuePolicy>,
    ) -> Self {
        Self {
            link,
            bandwidth_bytes_per_tick,
            request_virtual_network,
            response_virtual_network,
            credit_depth,
            router_stage,
            qos_queue_policy,
        }
    }

    pub fn link(&self) -> &str {
        &self.link
    }

    pub const fn bandwidth_bytes_per_tick(&self) -> u64 {
        self.bandwidth_bytes_per_tick
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
        self.router_stage.as_ref()
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
    let link = parts.link.expect("checked above");
    let bandwidth_bytes_per_tick =
        parts
            .bandwidth_bytes_per_tick
            .ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--fabric-bandwidth-bytes-per-tick",
            })?;
    let router_stage = run_fabric_router_stage_config(
        parts.router,
        parts.router_input_port,
        parts.router_output_port,
        parts.router_virtual_channel,
        parts.request_router_virtual_channel,
        parts.response_router_virtual_channel,
        parts.router_latency,
    )?;

    Ok(Some(RunFabricConfig::new(
        link,
        bandwidth_bytes_per_tick,
        parts.request_virtual_network.unwrap_or(0),
        parts.response_virtual_network.unwrap_or(0),
        parts.credit_depth,
        router_stage,
        parts.qos_queue_policy,
    )))
}

pub(super) fn parse_run_fabric_bandwidth(value: &str) -> Result<u64, Rem6CliError> {
    parse_positive_u64(value).ok_or_else(|| Rem6CliError::InvalidRunFabricBandwidth {
        value: value.to_string(),
    })
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

fn parse_run_fabric_router_port(value: &str) -> Result<u32, Rem6CliError> {
    parse_number(value)
        .and_then(|port| u32::try_from(port).ok())
        .ok_or_else(|| Rem6CliError::InvalidRunFabricRouterPort {
            value: value.to_string(),
        })
}

fn parse_run_fabric_router_virtual_channel(value: &str) -> Result<u16, Rem6CliError> {
    parse_number(value)
        .and_then(|channel| u16::try_from(channel).ok())
        .ok_or_else(|| Rem6CliError::InvalidRunFabricRouterVirtualChannel {
            value: value.to_string(),
        })
}

fn parse_run_fabric_router_latency(value: &str) -> Result<u64, Rem6CliError> {
    parse_positive_u64(value).ok_or_else(|| Rem6CliError::InvalidRunFabricRouterLatency {
        value: value.to_string(),
    })
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

fn run_fabric_router_stage_config(
    router: Option<String>,
    input_port: Option<u32>,
    output_port: Option<u32>,
    virtual_channel: Option<u16>,
    request_virtual_channel: Option<u16>,
    response_virtual_channel: Option<u16>,
    latency: Option<u64>,
) -> Result<Option<RunFabricRouterStageConfig>, Rem6CliError> {
    let has_router_stage = router.is_some()
        || input_port.is_some()
        || output_port.is_some()
        || virtual_channel.is_some()
        || request_virtual_channel.is_some()
        || response_virtual_channel.is_some()
        || latency.is_some();
    if !has_router_stage {
        return Ok(None);
    }
    let virtual_channel = virtual_channel.ok_or(Rem6CliError::MissingRequiredFlag {
        flag: "--fabric-router-virtual-channel",
    })?;
    Ok(Some(RunFabricRouterStageConfig::new(
        router.ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "--fabric-router",
        })?,
        input_port.ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "--fabric-router-input-port",
        })?,
        output_port.ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "--fabric-router-output-port",
        })?,
        virtual_channel,
        request_virtual_channel.unwrap_or(virtual_channel),
        response_virtual_channel.unwrap_or(virtual_channel),
        latency.ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "--fabric-router-latency",
        })?,
    )))
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

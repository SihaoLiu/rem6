use std::path::{Path, PathBuf};

use rem6_workload::WorkloadDataCacheProtocol;

use super::{
    load_trace_replay_file_config,
    parse::{parse_number, parse_positive_u64, required_value},
    parse_data_cache_protocol, trace_replay_file_config_from_args, CliDramMemoryProfile,
    Rem6TraceReplayConfig, StatsFormat, SuiteResourceSelector,
};
use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceReplayExternalAdapterKind {
    SystemC,
    Tlm,
    Sst,
}

impl TraceReplayExternalAdapterKind {
    pub(super) fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "systemc" => Ok(Self::SystemC),
            "tlm" => Ok(Self::Tlm),
            "sst" => Ok(Self::Sst),
            _ => Err(Rem6CliError::InvalidTraceReplayExternalAdapterKind {
                value: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SystemC => "systemc",
            Self::Tlm => "tlm",
            Self::Sst => "sst",
        }
    }
}

impl Rem6TraceReplayConfig {
    pub fn parse_args<I, S>(args: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let Some(command) = args.next() else {
            return Err(Rem6CliError::MissingCommand);
        };
        if command != "trace-replay" {
            return Err(Rem6CliError::UnsupportedCommand { command });
        }

        let remaining_args = args.collect::<Vec<_>>();
        let file_config = trace_replay_file_config_from_args(&remaining_args)?
            .map(|path| load_trace_replay_file_config(&path))
            .transpose()?
            .unwrap_or_default();

        let mut trace = file_config
            .trace
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut resource_config = file_config
            .resource_config
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut trace_resource = file_config
            .trace_resource
            .as_deref()
            .map(parse_trace_replay_resource)
            .transpose()?;
        let mut output = file_config
            .output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut stats_output = file_config
            .stats_output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut route = file_config.route;
        let mut memory_start = file_config.memory_start;
        let mut memory_size = file_config.memory_size;
        if memory_size == Some(0) {
            return Err(Rem6CliError::InvalidTraceReplayMemorySize {
                value: "0".to_string(),
            });
        }
        let mut max_tick = file_config.max_tick;
        let mut min_remote_delay = file_config.min_remote_delay.unwrap_or(1);
        if min_remote_delay == 0 {
            return Err(Rem6CliError::InvalidMinRemoteDelay {
                value: min_remote_delay.to_string(),
            });
        }
        let mut memory_route_delay = file_config.memory_route_delay;
        if memory_route_delay == Some(0) {
            return Err(Rem6CliError::InvalidMemoryRouteDelay {
                value: "0".to_string(),
            });
        }
        let mut tick_frequency = file_config.tick_frequency.unwrap_or(1_000);
        if tick_frequency == 0 {
            return Err(Rem6CliError::InvalidTraceReplayTickFrequency {
                value: "0".to_string(),
            });
        }
        let mut line_bytes = file_config.line_bytes.unwrap_or(64);
        if line_bytes == 0 {
            return Err(Rem6CliError::InvalidTraceReplayLineBytes {
                value: "0".to_string(),
            });
        }
        let mut agent = file_config.agent.unwrap_or(0);
        let mut control_partition = file_config.control_partition.unwrap_or(2);
        let mut data_cache_protocol = file_config
            .data_cache_protocol
            .as_deref()
            .map(|value| {
                parse_data_cache_protocol(value).ok_or_else(|| {
                    Rem6CliError::InvalidTraceReplayDataCacheProtocol {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut data_cache_dram_memory_profile = file_config
            .data_cache_dram_memory_profile
            .as_deref()
            .map(CliDramMemoryProfile::parse)
            .transpose()?;
        let mut fabric_link = file_config.fabric_link;
        let mut fabric_bandwidth_bytes_per_tick = file_config.fabric_bandwidth_bytes_per_tick;
        if fabric_bandwidth_bytes_per_tick == Some(0) {
            return Err(Rem6CliError::InvalidTraceReplayFabricBandwidth {
                value: "0".to_string(),
            });
        }
        let mut fabric_request_virtual_network = file_config.fabric_request_virtual_network;
        let mut fabric_response_virtual_network = file_config.fabric_response_virtual_network;
        let mut fabric_credit_depth = file_config.fabric_credit_depth;
        if fabric_credit_depth == Some(0) {
            return Err(Rem6CliError::InvalidTraceReplayFabricCreditDepth {
                value: "0".to_string(),
            });
        }
        let mut external_adapter_kind = file_config
            .external_adapter_kind
            .as_deref()
            .map(TraceReplayExternalAdapterKind::parse)
            .transpose()?;
        let mut external_adapter_endpoint = file_config.external_adapter_endpoint.clone();
        let mut stats_format = file_config
            .stats_format
            .as_deref()
            .map(StatsFormat::parse)
            .transpose()?
            .unwrap_or(StatsFormat::Json);
        let mut args = remaining_args.into_iter();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--config" => {
                    let _ = required_value(&flag, args.next())?;
                }
                "--trace" => {
                    trace = Some(PathBuf::from(required_value(&flag, args.next())?));
                    resource_config = None;
                    trace_resource = None;
                }
                "--resource-config" => {
                    resource_config = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--trace-resource" => {
                    let value = required_value(&flag, args.next())?;
                    trace_resource = Some(parse_trace_replay_resource(&value)?);
                }
                "--route" => {
                    route = Some(required_value(&flag, args.next())?);
                }
                "--memory-start" => {
                    let value = required_value(&flag, args.next())?;
                    memory_start = Some(parse_number(&value).ok_or_else(|| {
                        Rem6CliError::InvalidTraceReplayMemoryStart {
                            value: value.clone(),
                        }
                    })?);
                }
                "--memory-size" => {
                    let value = required_value(&flag, args.next())?;
                    memory_size = Some(parse_number(&value).filter(|size| *size > 0).ok_or_else(
                        || Rem6CliError::InvalidTraceReplayMemorySize {
                            value: value.clone(),
                        },
                    )?);
                }
                "--max-tick" => {
                    let value = required_value(&flag, args.next())?;
                    max_tick = Some(value.parse().map_err(|_| Rem6CliError::InvalidMaxTick {
                        value: value.clone(),
                    })?);
                }
                "--min-remote-delay" => {
                    let value = required_value(&flag, args.next())?;
                    min_remote_delay = parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidMinRemoteDelay {
                            value: value.clone(),
                        }
                    })?;
                }
                "--memory-route-delay" => {
                    let value = required_value(&flag, args.next())?;
                    memory_route_delay = Some(parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidMemoryRouteDelay {
                            value: value.clone(),
                        }
                    })?);
                }
                "--tick-frequency" => {
                    let value = required_value(&flag, args.next())?;
                    tick_frequency = parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidTraceReplayTickFrequency {
                            value: value.clone(),
                        }
                    })?;
                }
                "--line-bytes" => {
                    let value = required_value(&flag, args.next())?;
                    line_bytes = parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidTraceReplayLineBytes {
                            value: value.clone(),
                        }
                    })?;
                }
                "--agent" => {
                    let value = required_value(&flag, args.next())?;
                    agent = value
                        .parse()
                        .map_err(|_| Rem6CliError::InvalidTraceReplayAgent {
                            value: value.clone(),
                        })?;
                }
                "--control-partition" => {
                    let value = required_value(&flag, args.next())?;
                    control_partition = value.parse().map_err(|_| {
                        Rem6CliError::InvalidTraceReplayControlPartition {
                            value: value.clone(),
                        }
                    })?;
                }
                "--data-cache-protocol" => {
                    let value = required_value(&flag, args.next())?;
                    data_cache_protocol =
                        Some(parse_data_cache_protocol(&value).ok_or_else(|| {
                            Rem6CliError::InvalidTraceReplayDataCacheProtocol {
                                value: value.clone(),
                            }
                        })?);
                }
                "--data-cache-dram-memory-profile" => {
                    data_cache_dram_memory_profile = Some(CliDramMemoryProfile::parse(
                        &required_value(&flag, args.next())?,
                    )?);
                }
                "--fabric-link" => {
                    fabric_link = Some(required_value(&flag, args.next())?);
                }
                "--fabric-bandwidth-bytes-per-tick" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_bandwidth_bytes_per_tick =
                        Some(parse_positive_u64(&value).ok_or_else(|| {
                            Rem6CliError::InvalidTraceReplayFabricBandwidth {
                                value: value.clone(),
                            }
                        })?);
                }
                "--fabric-request-virtual-network" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_request_virtual_network = Some(parse_fabric_virtual_network(&value)?);
                }
                "--fabric-response-virtual-network" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_response_virtual_network = Some(parse_fabric_virtual_network(&value)?);
                }
                "--fabric-credit-depth" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_credit_depth = Some(parse_fabric_credit_depth(&value)?);
                }
                "--external-adapter-kind" => {
                    external_adapter_kind = Some(TraceReplayExternalAdapterKind::parse(
                        &required_value(&flag, args.next())?,
                    )?);
                }
                "--external-adapter-endpoint" => {
                    external_adapter_endpoint = Some(required_value(&flag, args.next())?);
                }
                "--stats-format" => {
                    stats_format = StatsFormat::parse(&required_value(&flag, args.next())?)?;
                }
                "--output" => {
                    output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--stats-output" => {
                    stats_output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                _ => return Err(Rem6CliError::UnknownFlag { flag }),
            }
        }

        if fabric_link.is_some() && fabric_bandwidth_bytes_per_tick.is_none() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--fabric-bandwidth-bytes-per-tick",
            });
        }
        if fabric_link.is_none() && fabric_bandwidth_bytes_per_tick.is_some() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--fabric-link",
            });
        }
        if fabric_link.is_none()
            && (fabric_request_virtual_network.is_some()
                || fabric_response_virtual_network.is_some()
                || fabric_credit_depth.is_some())
        {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--fabric-link",
            });
        }
        if external_adapter_kind.is_some() && external_adapter_endpoint.is_none() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--external-adapter-endpoint",
            });
        }
        if external_adapter_endpoint.is_some() && external_adapter_kind.is_none() {
            return Err(Rem6CliError::TraceReplayExternalAdapterEndpointRequiresKind);
        }
        if let (Some(output), Some(stats_output)) = (&output, &stats_output) {
            if output == stats_output {
                return Err(Rem6CliError::ConflictingOutputPaths {
                    path: output.to_path_buf(),
                });
            }
        }
        if trace_resource.is_some() && resource_config.is_none() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--resource-config",
            });
        }
        if data_cache_dram_memory_profile.is_some() && data_cache_protocol.is_none() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--data-cache-protocol",
            });
        }
        let memory_route_delay = memory_route_delay.unwrap_or(min_remote_delay);
        if memory_route_delay < min_remote_delay {
            return Err(Rem6CliError::MemoryRouteDelayBelowMinRemoteDelay {
                memory_route_delay,
                min_remote_delay,
            });
        }

        Ok(Self {
            trace: trace.or_else(|| resource_config.clone()).ok_or(
                Rem6CliError::MissingRequiredFlag {
                    flag: "--trace or --resource-config",
                },
            )?,
            resource_config,
            trace_resource,
            route: route.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--route" })?,
            memory_start: memory_start.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--memory-start",
            })?,
            memory_size: memory_size.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--memory-size",
            })?,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
            min_remote_delay,
            memory_route_delay,
            tick_frequency,
            line_bytes,
            agent,
            control_partition,
            data_cache_protocol,
            data_cache_dram_memory_profile,
            fabric_link,
            fabric_bandwidth_bytes_per_tick,
            fabric_request_virtual_network: fabric_request_virtual_network.unwrap_or(0),
            fabric_response_virtual_network: fabric_response_virtual_network.unwrap_or(0),
            fabric_credit_depth,
            external_adapter_kind,
            external_adapter_endpoint,
            stats_format,
            output,
            stats_output,
        })
    }

    pub fn trace(&self) -> &Path {
        &self.trace
    }

    pub fn resource_config(&self) -> Option<&Path> {
        self.resource_config.as_deref()
    }

    pub fn trace_resource(&self) -> Option<&SuiteResourceSelector> {
        self.trace_resource.as_ref()
    }

    pub fn trace_input(&self) -> String {
        self.resource_config
            .as_ref()
            .map(|path| format!("resource-config:{}", path.display()))
            .unwrap_or_else(|| self.trace.display().to_string())
    }

    pub fn route(&self) -> &str {
        &self.route
    }

    pub const fn memory_start(&self) -> u64 {
        self.memory_start
    }

    pub const fn memory_size(&self) -> u64 {
        self.memory_size
    }

    pub const fn max_tick(&self) -> u64 {
        self.max_tick
    }

    pub const fn min_remote_delay(&self) -> u64 {
        self.min_remote_delay
    }

    pub const fn memory_route_delay(&self) -> u64 {
        self.memory_route_delay
    }

    pub const fn tick_frequency(&self) -> u64 {
        self.tick_frequency
    }

    pub const fn line_bytes(&self) -> u64 {
        self.line_bytes
    }

    pub const fn agent(&self) -> u32 {
        self.agent
    }

    pub const fn control_partition(&self) -> u32 {
        self.control_partition
    }

    pub const fn data_cache_protocol(&self) -> Option<WorkloadDataCacheProtocol> {
        self.data_cache_protocol
    }

    pub const fn data_cache_dram_memory_profile(&self) -> Option<CliDramMemoryProfile> {
        self.data_cache_dram_memory_profile
    }

    pub fn fabric_link(&self) -> Option<&str> {
        self.fabric_link.as_deref()
    }

    pub const fn fabric_bandwidth_bytes_per_tick(&self) -> Option<u64> {
        self.fabric_bandwidth_bytes_per_tick
    }

    pub const fn fabric_request_virtual_network(&self) -> u16 {
        self.fabric_request_virtual_network
    }

    pub const fn fabric_response_virtual_network(&self) -> u16 {
        self.fabric_response_virtual_network
    }

    pub const fn fabric_credit_depth(&self) -> Option<u32> {
        self.fabric_credit_depth
    }

    pub const fn external_adapter_kind(&self) -> Option<TraceReplayExternalAdapterKind> {
        self.external_adapter_kind
    }

    pub fn external_adapter_endpoint(&self) -> Option<&str> {
        self.external_adapter_endpoint.as_deref()
    }

    pub const fn stats_format(&self) -> StatsFormat {
        self.stats_format
    }

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub fn stats_output(&self) -> Option<&Path> {
        self.stats_output.as_deref()
    }
}

fn parse_trace_replay_resource(value: &str) -> Result<SuiteResourceSelector, Rem6CliError> {
    SuiteResourceSelector::parse_source(value).ok_or_else(|| {
        Rem6CliError::InvalidTraceReplayResource {
            value: value.to_string(),
        }
    })
}

fn parse_fabric_virtual_network(value: &str) -> Result<u16, Rem6CliError> {
    parse_number(value)
        .and_then(|network| u16::try_from(network).ok())
        .ok_or_else(|| Rem6CliError::InvalidTraceReplayFabricVirtualNetwork {
            value: value.to_string(),
        })
}

fn parse_fabric_credit_depth(value: &str) -> Result<u32, Rem6CliError> {
    parse_positive_u64(value)
        .and_then(|depth| u32::try_from(depth).ok())
        .ok_or_else(|| Rem6CliError::InvalidTraceReplayFabricCreditDepth {
            value: value.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_replay_data_cache_dram_memory_profile_requires_data_cache_protocol() {
        let error = Rem6TraceReplayConfig::parse_args([
            "trace-replay",
            "--trace",
            "trace.pb",
            "--route",
            "cpu0.data",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "0x1000",
            "--max-tick",
            "64",
            "--data-cache-dram-memory-profile",
            "hbm",
        ])
        .unwrap_err();

        assert!(matches!(
            error,
            Rem6CliError::MissingRequiredFlag {
                flag: "--data-cache-protocol"
            }
        ));
    }
}

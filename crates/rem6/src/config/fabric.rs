use crate::Rem6CliError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunFabricConfig {
    link: String,
    bandwidth_bytes_per_tick: u64,
    request_virtual_network: u16,
    response_virtual_network: u16,
    credit_depth: Option<u32>,
}

impl RunFabricConfig {
    pub(super) fn new(
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
}

pub(super) fn run_fabric_config_from_parts(
    link: Option<String>,
    bandwidth_bytes_per_tick: Option<u64>,
    request_virtual_network: Option<u16>,
    response_virtual_network: Option<u16>,
    credit_depth: Option<u32>,
) -> Result<Option<RunFabricConfig>, Rem6CliError> {
    let Some(link) = link else {
        if bandwidth_bytes_per_tick.is_some()
            || request_virtual_network.is_some()
            || response_virtual_network.is_some()
            || credit_depth.is_some()
        {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--fabric-link",
            });
        }
        return Ok(None);
    };
    let bandwidth_bytes_per_tick =
        bandwidth_bytes_per_tick.ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "--fabric-bandwidth-bytes-per-tick",
        })?;

    Ok(Some(RunFabricConfig::new(
        link,
        bandwidth_bytes_per_tick,
        request_virtual_network.unwrap_or(0),
        response_virtual_network.unwrap_or(0),
        credit_depth,
    )))
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
